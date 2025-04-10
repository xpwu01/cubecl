use std::marker::PhantomData;

use crate::matmul::components::global::Quantization;
use crate::matmul::components::global::load::SyncFullLoadingStrategy;
use crate::matmul::components::{
    FormattedConfigError, Ident, InputIdent, InvalidConfigError, MatmulPrecision,
};
use crate::matmul::components::{
    global::{GlobalConfig, LoadingValidation, tensor_view::TensorReader},
    stage::{ContiguousTilingLayout, Stage, TilingOrder},
};
use cubecl_core as cubecl;
use cubecl_core::prelude::*;
use cubecl_std::{CubeOption, CubeOptionExpand};

use super::{LoadingJob, LoadingJobConfig};

#[derive(CubeType, Clone, Copy)]
/// Loads the content of all tiles in the tensor view using
/// one plane per tile.
pub struct LoadingStrategy<T: TilingOrder> {
    #[cube(comptime)]
    tiling_order: PhantomData<T>,
}

impl<T: TilingOrder> LoadingValidation for LoadingStrategy<T> {
    fn check<C: GlobalConfig>(config: &C, ident: Ident) -> Result<(), InvalidConfigError> {
        let tiling = config.tiling_dimensions(ident);
        let line_size = config.global_line_size(ident);

        let num_planes = config.num_planes();
        let num_tiles = tiling.tile_count();

        if num_planes != num_tiles {
            return Err(FormattedConfigError::new(move || {
                format!(
                    "Number of planes {:?} must equal number of tiles {:?} for tilewise loading.",
                    num_planes, num_tiles,
                )
            }));
        }

        if line_size != config.stage_line_size(ident) {
            return Err(Box::new(
                "Global and stage line sizes must match for tilewise loading.",
            ));
        }

        Ok(())
    }
}

#[cube]
impl<T: TilingOrder> SyncFullLoadingStrategy for LoadingStrategy<T> {
    type TilingLayout = ContiguousTilingLayout<T>;
    type Job<MP: MatmulPrecision> = Job<MP>;

    fn new_job<MP: MatmulPrecision, G: GlobalConfig>(
        quantization: CubeOption<Quantization<MP>>,
        #[comptime] input_ident: InputIdent,
        #[comptime] config: G,
    ) -> Self::Job<MP> {
        let tiling = config.tiling_dimensions(input_ident);
        let line_size = config.global_line_size(input_ident);

        let num_lines_per_tile = comptime!(tiling.tile_size() / line_size);

        let nth_tile = UNIT_POS_Y;
        let offset_base = num_lines_per_tile * nth_tile;

        let num_tasks = num_lines_per_tile / config.plane_dim();

        let tile = ContiguousTilingLayout::<T>::to_x_y::<G::SmmConfig>(
            nth_tile,
            input_ident.as_ident(),
            config.to_smm_config(),
        );

        Job::<MP> {
            tile,
            offset_base,
            quantization,
            job_config: comptime!(JobConfig {
                num_tasks,
                line_size,
                input_ident,
            }),
        }
    }
}

#[derive(CubeType, Clone, Copy)]
pub struct Job<MP: MatmulPrecision> {
    tile: (u32, u32),
    offset_base: u32,

    quantization: CubeOption<Quantization<MP>>,

    #[cube(comptime)]
    job_config: JobConfig,
}

#[derive(Copy, Clone)]
pub struct JobConfig {
    num_tasks: u32,
    line_size: u32,
    input_ident: InputIdent,
}

impl<MP: MatmulPrecision, TO: TilingOrder> LoadingJobConfig<MP, ContiguousTilingLayout<TO>, Job<MP>>
    for JobConfig
{
    fn len(job: &Job<MP>) -> u32 {
        job.job_config.num_tasks
    }

    fn __expand_len(
        _context: &mut cubecl_core::prelude::Scope,
        job: <Job<MP> as cubecl_core::prelude::CubeType>::ExpandType,
    ) -> u32 {
        job.job_config.num_tasks
    }
}

#[cube]
impl<MP: MatmulPrecision, TO: TilingOrder> LoadingJob<MP, ContiguousTilingLayout<TO>> for Job<MP> {
    type LoadingJobConfig = JobConfig;

    fn execute_task<G: GlobalConfig>(
        this: &mut Self,
        task_id: u32,
        tensor_reader: &TensorReader<MP::EI>,
        stage: &mut Stage<MP::ES, ContiguousTilingLayout<TO>>,
        #[comptime] config: G,
    ) {
        let pos_within_tile = task_id * comptime!(config.plane_dim()) + UNIT_POS_X;

        let line_read = tensor_reader.load_coalesced_in_tile::<G>(
            this.tile.0,
            this.tile.1,
            pos_within_tile * this.job_config.line_size,
            this.job_config.input_ident,
            config,
        );

        let offset = this.offset_base + pos_within_tile;

        stage.as_slice_mut()[offset] = match this.quantization {
            CubeOption::Some(quantization) => quantization.dequantize(line_read),
            CubeOption::None => Line::cast_from(line_read),
        }
    }
}
