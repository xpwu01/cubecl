use crate::matmul::components::{
    Ident, InputIdent, InvalidConfigError, MatmulPrecision, MatrixLayout,
    global::{
        CopyMechanism, GlobalConfig, LoadingValidation, Quantization,
        load::AsyncFullLoadingStrategy,
        tensor_view::{TensorReader, Window},
    },
    stage::{Stage, StridedTilingLayout},
};
use cubecl_core::prelude::*;
use cubecl_core::{self as cubecl, prelude::barrier::BarrierLevel};
use cubecl_std::{CubeOption, div_ceil};

use super::{AsyncLoadingJob, AsyncLoadingJobConfig};

#[derive(CubeType, Clone, Copy)]
/// Executes one memcpy_async call per contiguous slice.
/// The goal is to reduce the total number of memcpy_async calls, though it may result in idle threads.
pub struct LoadingStrategy {}

impl LoadingValidation for LoadingStrategy {
    fn check<C: GlobalConfig>(_config: &C, _ident: Ident) -> Result<(), InvalidConfigError> {
        Ok(())
    }
}

#[cube]
impl AsyncFullLoadingStrategy for LoadingStrategy {
    type TilingLayout = StridedTilingLayout;
    type Job<MP: MatmulPrecision> = Job;

    fn new_job<MP: MatmulPrecision, G: GlobalConfig>(
        quantization: CubeOption<Quantization<MP>>,
        #[comptime] input_ident: InputIdent,
        #[comptime] config: G,
    ) -> Job {
        comptime! {
            if quantization.is_some() {
                panic!("Quantization not supported on async loaders.")
            }
        }

        let matrix_layout = config.matrix_layout(input_ident);
        let tiling_dimensions = config.tiling_dimensions(input_ident);

        let num_slices = match matrix_layout {
            MatrixLayout::RowMajor => tiling_dimensions.total_row(),
            MatrixLayout::ColMajor => tiling_dimensions.total_col(),
        };
        let unit_count = config.plane_dim() * config.num_planes();

        let num_tasks = comptime!(div_ceil(num_slices, unit_count));

        Job {
            job_config: comptime!(JobConfig {
                num_tasks,
                unit_count,
                num_slices,
                input_ident,
            }),
        }
    }

    fn barrier_level() -> BarrierLevel {
        BarrierLevel::cube_manual(0u32)
    }
}

#[derive(CubeType, Clone, Copy)]
pub struct Job {
    #[cube(comptime)]
    job_config: JobConfig,
}

#[derive(Clone, Copy)]
pub struct JobConfig {
    num_tasks: u32,
    unit_count: u32,
    num_slices: u32,
    input_ident: InputIdent,
}

impl<MP: MatmulPrecision> AsyncLoadingJobConfig<MP, StridedTilingLayout, Job> for JobConfig {
    fn len(job: &Job) -> u32 {
        job.job_config.num_tasks
    }

    fn __expand_len(
        _context: &mut cubecl_core::prelude::Scope,
        job: <Job as cubecl_core::prelude::CubeType>::ExpandType,
    ) -> u32 {
        job.job_config.num_tasks
    }
}

#[cube]
impl<MP: MatmulPrecision> AsyncLoadingJob<MP, StridedTilingLayout> for Job {
    type LoadingJobConfig = JobConfig;

    fn execute_task<CM: CopyMechanism<MP::ES>, G: GlobalConfig>(
        this: &mut Self,
        task_id: u32,
        tensor_reader: &TensorReader<MP::EI>,
        stage: &mut Stage<MP::ES, StridedTilingLayout>,
        mechanism: &CM,
        #[comptime] config: G,
    ) {
        let jc = this.job_config;
        let nth_slice = jc.unit_count * task_id + UNIT_POS;

        #[allow(clippy::collapsible_else_if)]
        if comptime!(jc.num_slices % jc.unit_count == 0) {
            load_nth_slice::<MP::EI, MP::ES, CM, G>(
                nth_slice,
                tensor_reader,
                stage,
                mechanism,
                jc.input_ident,
                config,
            );
        } else {
            if nth_slice < jc.num_slices {
                load_nth_slice::<MP::EI, MP::ES, CM, G>(
                    nth_slice,
                    tensor_reader,
                    stage,
                    mechanism,
                    jc.input_ident,
                    config,
                );
            }
        };
    }
}

#[cube]
fn load_nth_slice<EG: Numeric, ES: Numeric, CM: CopyMechanism<ES>, G: GlobalConfig>(
    nth_slice: u32,
    tensor_reader: &TensorReader<EG>,
    stage: &mut Stage<ES, StridedTilingLayout>,
    mechanism: &CM,
    #[comptime] input_ident: InputIdent,
    #[comptime] config: G,
) {
    let window: Window<EG> =
        tensor_reader.load_window_in_stage::<G>(nth_slice, input_ident, config);
    let mut destination: SliceMut<Line<ES>> = StridedTilingLayout::nth_slice::<ES, G::SmmConfig>(
        stage,
        nth_slice,
        comptime!(input_ident.as_ident()),
        config.to_smm_config(),
    );

    CM::memcpy_async(
        mechanism,
        &window.slice.try_cast_unchecked(),
        &mut destination,
    );
}
