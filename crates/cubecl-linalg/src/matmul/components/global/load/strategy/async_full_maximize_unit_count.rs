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
use cubecl_std::CubeOption;

use super::{AsyncLoadingJob, AsyncLoadingJobConfig};

#[derive(CubeType, Clone, Copy)]
/// Executes one memcpy_async call per unit.
/// The objective is to reduce branching, prioritizing this over maximizing memory slice length.
pub struct LoadingStrategy {}

impl LoadingValidation for LoadingStrategy {
    fn check<C: GlobalConfig>(config: &C, ident: Ident) -> Result<(), InvalidConfigError> {
        let matrix_layout = config.matrix_layout(ident);
        let tiling_dimensions = config.tiling_dimensions(ident);
        let line_size = config.global_line_size(ident);

        let (num_slices, slice_length) = match matrix_layout {
            MatrixLayout::RowMajor => (
                tiling_dimensions.total_row(),
                tiling_dimensions.total_col() / line_size,
            ),
            MatrixLayout::ColMajor => (
                tiling_dimensions.total_col(),
                tiling_dimensions.total_row() / line_size,
            ),
        };
        let unit_count = config.plane_dim() * config.num_planes();

        if unit_count % num_slices != 0 {
            return Err(Box::new(
                "Number of slices must divide number of units evenly",
            ));
        }
        if slice_length % (unit_count / num_slices) != 0 {
            return Err(Box::new(
                "Number of units per slice must divide slice length evenly",
            ));
        }

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
        let line_size = config.global_line_size(input_ident);

        let (num_slices, slice_length) = match matrix_layout {
            MatrixLayout::RowMajor => (
                tiling_dimensions.total_row(),
                tiling_dimensions.total_col() / line_size,
            ),
            MatrixLayout::ColMajor => (
                tiling_dimensions.total_col(),
                tiling_dimensions.total_row() / line_size,
            ),
        };

        let unit_count = config.plane_dim() * config.num_planes();

        let units_per_slice = comptime!(unit_count / num_slices);
        let nth_slice = UNIT_POS / units_per_slice;

        let segment_length = comptime!(slice_length / units_per_slice);
        let nth_segment = UNIT_POS % units_per_slice;

        Job {
            nth_slice,
            nth_segment,
            job_config: comptime!(JobConfig {
                segment_length,
                input_ident
            }),
        }
    }

    fn barrier_level() -> BarrierLevel {
        BarrierLevel::cube_manual(0u32)
    }
}

#[derive(CubeType, Clone, Copy)]
pub struct Job {
    nth_slice: u32,
    nth_segment: u32,
    #[cube(comptime)]
    job_config: JobConfig,
}

#[derive(Clone, Copy)]
pub struct JobConfig {
    segment_length: u32,
    input_ident: InputIdent,
}

impl<MP: MatmulPrecision> AsyncLoadingJobConfig<MP, StridedTilingLayout, Job> for JobConfig {
    fn len(_job: &Job) -> u32 {
        1
    }

    fn __expand_len(
        _context: &mut cubecl_core::prelude::Scope,
        _job: <Job as cubecl_core::prelude::CubeType>::ExpandType,
    ) -> u32 {
        1
    }
}

#[cube]
impl<MP: MatmulPrecision> AsyncLoadingJob<MP, StridedTilingLayout> for Job {
    type LoadingJobConfig = JobConfig;

    fn execute_task<CM: CopyMechanism<MP::ES>, G: GlobalConfig>(
        this: &mut Self,
        _task_id: u32,
        tensor_reader: &TensorReader<MP::EI>,
        stage: &mut Stage<MP::ES, StridedTilingLayout>,
        mechanism: &CM,
        #[comptime] config: G,
    ) {
        let jc = this.job_config;

        let mut destination: SliceMut<Line<MP::ES>> =
            StridedTilingLayout::nth_slice::<MP::ES, G::SmmConfig>(
                stage,
                this.nth_slice,
                comptime!(jc.input_ident.as_ident()),
                config.to_smm_config(),
            );

        let window: Window<MP::EI> =
            tensor_reader.load_window_in_stage::<G>(this.nth_slice, jc.input_ident, config);
        let seg_start = Min::min(this.nth_segment * jc.segment_length, window.size);
        let seg_end = Min::min((this.nth_segment + 1) * jc.segment_length, window.size);

        let src_segment = window.slice.slice(seg_start, seg_end);
        let mut dest_segment = destination.slice_mut(seg_start, seg_end);

        CM::memcpy_async(
            mechanism,
            &src_segment.try_cast_unchecked(),
            &mut dest_segment,
        );
    }
}
