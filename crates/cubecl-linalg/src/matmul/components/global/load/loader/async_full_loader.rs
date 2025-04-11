use std::marker::PhantomData;

use crate::matmul::components::global::load::AsyncLoadingJob;
use crate::matmul::components::global::tensor_view::TensorReader;
use crate::matmul::components::global::{CopyMechanism, GlobalConfig, LoadingValidation};
use crate::matmul::components::global::{Quantization, single_stage};
use crate::matmul::components::stage::FullReader;
use crate::matmul::components::stage::TilingLayout;
use crate::matmul::components::stage::{self, Stage};
use crate::matmul::components::{Ident, InputIdent, MatmulPrecision, global};
use cubecl_core as cubecl;
use cubecl_core::prelude::barrier::BarrierLevel;
use cubecl_core::prelude::*;
use cubecl_std::CubeOption;
use cubecl_std::tensor::r#virtual::VirtualTensor;

#[cube]
/// A strategy for fully and asynchronously loading a stage.
pub trait AsyncFullLoadingStrategy: 'static + Send + Sync + Clone + LoadingValidation {
    /// The layout describing how data is tiled across the stage.
    type TilingLayout: TilingLayout;

    /// The [LoadingJob] for this strategy.
    type Job<MP: MatmulPrecision>: AsyncLoadingJob<MP, Self::TilingLayout>;

    /// Returns the job with preliminary calculations done.
    fn new_job<MP: MatmulPrecision, G: GlobalConfig>(
        quantization: CubeOption<Quantization<MP>>,
        #[comptime] ident: InputIdent,
        #[comptime] config: G,
    ) -> Self::Job<MP>;

    /// The barrier level at which the copy mechanism works
    fn barrier_level() -> BarrierLevel;
}

#[derive(CubeType)]
pub struct AsyncLoader<
    MP: MatmulPrecision,
    CM: CopyMechanism<MP::ES>,
    S: stage::StageConfig,
    L: AsyncFullLoadingStrategy,
> {
    tensor_reader: TensorReader<MP::EI>,
    stage: Stage<MP::ES, L::TilingLayout>,
    loading_job: L::Job<MP>,
    #[cube(comptime)]
    ident: InputIdent,
    #[cube(comptime)]
    _phantom: PhantomData<(S, L, CM)>,
}

#[cube]
impl<
    MP: MatmulPrecision,
    CM: CopyMechanism<MP::ES>,
    S: stage::StageConfig,
    L: AsyncFullLoadingStrategy,
> AsyncLoader<MP, CM, S, L>
{
    pub fn new<G: global::GlobalConfig>(
        tensor: VirtualTensor<MP::EI>,
        x_offset: u32,
        y_offset: u32,
        batch_offset: u32,
        quantization: CubeOption<Quantization<MP>>,
        #[comptime] ident: InputIdent,
        #[comptime] config: G,
    ) -> Self {
        comptime! {
            if quantization.is_some() {
                todo!();
            }
        }

        let mut stage = Stage::new::<G::SmmConfig>(ident.as_ident(), config.to_smm_config());
        let loading_job = L::new_job::<MP, G>(quantization, ident, config);

        match ident {
            InputIdent::Lhs =>
            {
                #[allow(clippy::collapsible_if)]
                if config.check_row_bounds(ident) {
                    if x_offset
                        > tensor.shape(tensor.rank() - 2)
                            - config.tiling_dimensions(Ident::Lhs).total_row()
                    {
                        stage.clear::<G::SmmConfig>(ident, config.to_smm_config());
                    }
                }
            }
            InputIdent::Rhs =>
            {
                #[allow(clippy::collapsible_if)]
                if config.check_col_bounds(ident) {
                    if y_offset
                        > tensor.shape(tensor.rank() - 1)
                            - config.tiling_dimensions(Ident::Rhs).total_col()
                    {
                        stage.clear::<G::SmmConfig>(ident, config.to_smm_config());
                    }
                }
            }
        }

        let tensor_reader = TensorReader::new(tensor, x_offset, y_offset, batch_offset);

        AsyncLoader::<MP, CM, S, L> {
            tensor_reader,
            stage,
            loading_job,
            ident,
            _phantom: PhantomData::<(S, L, CM)>,
        }
    }

    pub fn fill_stage(
        this: &mut Self,
        mechanism: &CM,
        #[comptime] config: single_stage::Config<S>,
    ) {
        let len = L::Job::task_count(&this.loading_job);
        for task_id in 0..len {
            L::Job::<MP>::execute_task::<CM, single_stage::Config<S>>(
                &mut this.loading_job,
                task_id,
                &this.tensor_reader,
                &mut this.stage,
                mechanism,
                config,
            );
        }
    }

    pub fn clear_stage(this: &mut Self, #[comptime] config: single_stage::Config<S>) {
        this.stage.clear::<S>(this.ident, config.to_smm_config())
    }

    pub fn reader(this: &Self) -> FullReader<MP::ES, L::TilingLayout> {
        FullReader::new(this.stage, this.ident)
    }

    pub fn advance_view(this: &mut Self, k_offset: u32) {
        this.tensor_reader.update_view(k_offset, this.ident);
    }
}
