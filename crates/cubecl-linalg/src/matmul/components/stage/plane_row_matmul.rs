use crate::matmul::components::global::AccumulatorLoader;
use crate::matmul::components::stage::shared::CommonStageConfig;
use crate::matmul::components::stage::shared::{RhsTile, RhsTileExpand};
use crate::matmul::components::stage::{NoEvent, StageBuffering, StageEvent, StageEventListener};
use crate::matmul::components::stage::{Reader, ReaderFamily};
use crate::matmul::components::stage::{StageConfig, StageMatmul, StageMatmulFamily, TilingLayout};
use crate::matmul::components::tile::TileMatmul;
use crate::matmul::components::tile::TileMatmulFamily;
use crate::matmul::components::{
    CompleteStageTiling, InvalidConfigError, MatmulConfigFactory, MatmulPrecision, MatmulSize,
};
use crate::matmul::components::{Ident, MatmulProblem, global, stage::StageWriter, tile};
use crate::matmul::kernels::MatmulAvailabilityError;
use core::marker::PhantomData;
use cubecl::prelude::*;
use cubecl_core as cubecl;

pub struct PlaneRowMatmulFamily<TMM: TileMatmulFamily, RF: ReaderFamily> {
    _phantom: PhantomData<(TMM, RF)>,
}

impl<TMM: TileMatmulFamily, RF: ReaderFamily> StageMatmulFamily for PlaneRowMatmulFamily<TMM, RF> {
    fn stage_shape(config: &Self::Config) -> MatmulSize {
        config.tiling.total_shape()
    }

    fn tile_count(config: &Self::Config) -> MatmulSize {
        config.tiling.tile_count
    }

    type LhsReader = RF;
    type RhsReader = RF;
    type Matmul<MP: MatmulPrecision, TL: TilingLayout, TR: TilingLayout> =
        PlaneRowMatmul<MP, TMM::Matmul<MP>, RF::Reader<MP::ES, TL>, RF::Reader<MP::ES, TR>>;
}

impl<TMM: TileMatmulFamily, RF: ReaderFamily> MatmulConfigFactory
    for PlaneRowMatmulFamily<TMM, RF>
{
    type Input = (CompleteStageTiling, StageBuffering);
    type Config = CommonStageConfig<TMM::Config>;

    fn check_config(config: &Self::Config) -> Result<(), InvalidConfigError> {
        check_num_planes(
            config.tiling_dimensions(Ident::Lhs).tile_count_row(),
            config.num_planes(),
        )?;
        TMM::check_config(&config.to_tmm_config())
    }

    fn check_availability<R: Runtime, MP: MatmulPrecision>(
        client: &ComputeClient<R::Server, R::Channel>,
        config: &Self::Config,
    ) -> Result<(), MatmulAvailabilityError> {
        TMM::check_availability::<R, MP>(client, &config.tmm_config)
    }

    fn make_config(
        input: Self::Input,
        problem: &MatmulProblem,
        cube_dim: &CubeDim,
        cube_count: &CubeCount,
        quantized: bool,
    ) -> Self::Config {
        let tile_shape = input.0.tile_shape;
        let tile_count = input.0.tile_count;

        let tmm_config = TMM::make_config(tile_shape, problem, cube_dim, cube_count, quantized);

        let tiling = CompleteStageTiling {
            tile_shape,
            tile_count,
        };

        CommonStageConfig::new(tmm_config, tiling, cube_dim.y, quantized, input.1)
    }
}

/// Performs matrix multiplication at the stage level, where each plane is responsible for a row of tiles:
/// - One plane per tile in m dimension,
/// - One accumulator per tile in n dimension
///
/// # Assumptions
/// - There are as many planes as the stage size in m
pub struct PlaneRowMatmul<
    MP: MatmulPrecision,
    TMM: tile::TileMatmul<MP>,
    RL: Reader<MP::ES>,
    RR: Reader<MP::ES>,
> {
    _phantom: PhantomData<(MP, TMM, RL, RR)>,
}

#[cube]
impl<MP, TMM, RL, RR> StageMatmul<MP> for PlaneRowMatmul<MP, TMM, RL, RR>
where
    MP: MatmulPrecision,
    TMM: tile::TileMatmul<MP>,
    RL: Reader<MP::ES>,
    RR: Reader<MP::ES>,
{
    type Config = CommonStageConfig<TMM::Config>;

    type LhsReader = RL;
    type RhsReader = RR;
    type Accumulator = Sequence<TMM::Accumulator>;
    type LhsTile = TMM::Lhs;
    type RhsTile = RhsTile<TMM::Rhs>;

    fn execute(
        lhs_reader: &RL,
        rhs_reader: &RR,
        lhs_fragment: &mut Self::LhsTile,
        rhs_fragments: &mut Self::RhsTile,
        acc: &mut Self::Accumulator,
        #[comptime] config: Self::Config,
    ) {
        Self::execute_with_listener::<NoEvent>(
            lhs_reader,
            rhs_reader,
            lhs_fragment,
            rhs_fragments,
            acc,
            config,
            NoEvent::new(),
        )
    }

    fn execute_with_listener<SEL: StageEventListener>(
        lhs_reader: &RL,
        rhs_reader: &RR,
        lhs_fragment: &mut Self::LhsTile,
        rhs_fragments: &mut Self::RhsTile,
        acc: &mut Self::Accumulator,
        #[comptime] config: Self::Config,
        listener: SEL,
    ) {
        match rhs_fragments {
            RhsTile::Single(rhs_fragment) => Self::execute_single_buffer::<SEL>(
                lhs_reader,
                rhs_reader,
                lhs_fragment,
                rhs_fragment,
                acc,
                config,
                listener,
            ),
            RhsTile::Double(rhs_fragments) => Self::execute_double_buffer::<SEL>(
                lhs_reader,
                rhs_reader,
                lhs_fragment,
                rhs_fragments,
                acc,
                config,
                listener,
            ),
        }
    }

    fn init_tile_inputs(#[comptime] config: Self::Config) -> (Self::LhsTile, Self::RhsTile) {
        let tmm_config = config.to_tmm_config();
        (
            TMM::allocate_lhs(tmm_config),
            match config.buffering() {
                StageBuffering::Single => RhsTile::new_Single(TMM::allocate_rhs(tmm_config)),
                StageBuffering::Double => RhsTile::new_Double((
                    TMM::allocate_rhs(tmm_config),
                    TMM::allocate_rhs(tmm_config),
                )),
            },
        )
    }

    fn read_accumulator<SW: StageWriter<MP::EO>, G: global::GlobalConfig>(
        acc: &Self::Accumulator,
        out: &mut SW,
        #[comptime] stage_config: Self::Config,
        #[comptime] global_config: G,
    ) {
        let out_smem_line_size = global_config.stage_line_size(Ident::Out);
        let num_tile_lines =
            stage_config.tiling_dimensions(Ident::Out).tile_size() / out_smem_line_size;

        let start = num_tile_lines * UNIT_POS_Y;

        let mut out_smem = SharedMemory::<MP::EO>::new_lined(
            num_tile_lines * stage_config.num_planes(),
            out_smem_line_size,
        );
        let mut smem_slice = out_smem.slice_mut(start, start + num_tile_lines);

        #[unroll]
        for accumulator_iter in 0..acc.len() {
            let accumulator = acc.index(accumulator_iter);
            TMM::read_accumulator(accumulator, &mut smem_slice, stage_config.to_tmm_config());
            SW::write::<MP::EO, G>(
                out,
                smem_slice.to_slice(),
                UNIT_POS_Y,
                accumulator_iter,
                global_config,
            );
        }
    }

    fn init_accumulator(#[comptime] config: Self::Config) -> Self::Accumulator {
        let mut tmm_accumulators = Sequence::<TMM::Accumulator>::new();

        #[unroll]
        for _ in 0..config.tile_count().n {
            tmm_accumulators.push(TMM::allocate_accumulator(config.to_tmm_config()));
        }

        tmm_accumulators
    }

    fn zero_accumulator(acc: &mut Self::Accumulator, #[comptime] config: Self::Config) {
        #[unroll]
        for i in 0..config.tile_count().n {
            TMM::zero_accumulator(acc.index_mut(i), config.to_tmm_config());
        }
    }

    fn fill_accumulator<L: AccumulatorLoader<MP, Self::Config>>(
        loader: &mut L,
        acc: &mut Self::Accumulator,
        #[comptime] config: Self::Config,
    ) {
        #[unroll]
        for i in 0..config.tile_count().n {
            let acc = acc.index_mut(i);
            L::load::<TMM>(loader, acc, i, config.to_tmm_config());
        }
    }
}

fn check_num_planes(
    expected_num_planes: u32,
    actual_num_planes: u32,
) -> Result<(), InvalidConfigError> {
    if expected_num_planes != actual_num_planes {
        return Err(Box::new("Error: Expected {expected_num_planes} planes, but found {actual_num_planes}. 
        The number of planes is equal to cube dimension y which should be set to {expected_num_planes}."));
    }

    Ok(())
}

#[cube]
impl<MP, TMM, RL, RR> PlaneRowMatmul<MP, TMM, RL, RR>
where
    MP: MatmulPrecision,
    TMM: TileMatmul<MP>,
    RL: Reader<MP::ES>,
    RR: Reader<MP::ES>,
{
    // Execute stage matmul with a single buffer for rhs.
    fn execute_single_buffer<SEL: StageEventListener>(
        lhs_reader: &RL,
        rhs_reader: &RR,
        lhs_fragment: &mut TMM::Lhs,
        rhs_fragment: &mut TMM::Rhs,
        acc: &mut <Self as StageMatmul<MP>>::Accumulator,
        #[comptime] config: <Self as StageMatmul<MP>>::Config,
        mut listener: SEL,
    ) {
        SEL::on_event(&mut listener, StageEvent::Begin);

        let k_iterations = comptime!(RL::num_k_iterations(config));
        let acc_iterations = acc.len();
        let acc_total_iterations = comptime![k_iterations * acc_iterations];

        let mut k_iter = comptime![0u32];

        #[allow(clippy::explicit_counter_loop)]
        #[unroll]
        for _ in 0..k_iterations {
            let tile_lhs = RL::read_tile::<TMM::Config>(lhs_reader, UNIT_POS_Y, k_iter, config);
            TMM::fill_lhs(&tile_lhs, lhs_fragment, config.to_tmm_config());
            SEL::on_event(
                &mut listener,
                comptime![StageEvent::LhsLoaded {
                    current: k_iter,
                    total: k_iterations
                }],
            );

            let mut acc_iter = comptime![0u32];

            #[unroll]
            #[allow(clippy::explicit_counter_loop)]
            for _ in 0..acc_iterations {
                let current = comptime![k_iter * acc_iterations + acc_iter];
                let rhs_tile_next =
                    RR::read_tile::<TMM::Config>(rhs_reader, k_iter, acc_iter, config);
                TMM::fill_rhs(&rhs_tile_next, rhs_fragment, config.to_tmm_config());
                SEL::on_event(
                    &mut listener,
                    comptime![StageEvent::RhsLoaded {
                        current,
                        total: acc_total_iterations
                    }],
                );

                let accumulator = acc.index_mut(acc_iter);
                TMM::execute(
                    lhs_fragment,
                    rhs_fragment,
                    accumulator,
                    config.to_tmm_config(),
                );
                SEL::on_event(
                    &mut listener,
                    comptime![StageEvent::TmmCompleted {
                        current,
                        total: acc_total_iterations
                    }],
                );

                comptime![acc_iter += 1];
            }

            comptime![k_iter += 1];
        }

        SEL::on_event(&mut listener, comptime!(StageEvent::Finish));
    }

    // Execute stage matmul with two alternating buffers for rhs.
    fn execute_double_buffer<SEL: StageEventListener>(
        lhs_reader: &RL,
        rhs_reader: &RR,
        lhs_fragment: &mut TMM::Lhs,
        rhs_fragments: &mut (TMM::Rhs, TMM::Rhs),
        acc: &mut <Self as StageMatmul<MP>>::Accumulator,
        #[comptime] config: <Self as StageMatmul<MP>>::Config,
        mut listener: SEL,
    ) {
        SEL::on_event(&mut listener, StageEvent::Begin);

        let k_iterations = comptime!(RL::num_k_iterations(config));
        let num_accumulators = acc.len();
        let acc_total_iterations = comptime![k_iterations * num_accumulators];

        let mut k_iter = comptime![0u32];

        #[allow(clippy::explicit_counter_loop)]
        #[unroll]
        for _ in 0..k_iterations {
            let tile_lhs = RL::read_tile::<TMM::Config>(lhs_reader, UNIT_POS_Y, k_iter, config);
            TMM::fill_lhs(&tile_lhs, lhs_fragment, config.to_tmm_config());
            SEL::on_event(
                &mut listener,
                comptime![StageEvent::LhsLoaded {
                    current: k_iter,
                    total: k_iterations
                }],
            );

            let mut acc_iter = comptime![0u32];

            let rhs_tile_first = RR::read_tile::<TMM::Config>(rhs_reader, k_iter, acc_iter, config);
            TMM::fill_rhs(
                &rhs_tile_first,
                &mut rhs_fragments.0,
                config.to_tmm_config(),
            );
            SEL::on_event(
                &mut listener,
                comptime!(StageEvent::RhsLoaded {
                    current: k_iter * num_accumulators,
                    total: acc_total_iterations
                }),
            );

            #[unroll]
            #[allow(clippy::explicit_counter_loop)]
            for _ in 1..num_accumulators {
                let current_computation = comptime![k_iter * num_accumulators + acc_iter];
                let (current, next) = if comptime! {acc_iter % 2 == 0} {
                    (&mut rhs_fragments.0, &mut rhs_fragments.1)
                } else {
                    (&mut rhs_fragments.1, &mut rhs_fragments.0)
                };

                let rhs_tile_next = RR::read_tile::<TMM::Config>(
                    rhs_reader,
                    k_iter,
                    comptime![acc_iter + 1],
                    config,
                );
                TMM::fill_rhs(&rhs_tile_next, next, config.to_tmm_config());
                SEL::on_event(
                    &mut listener,
                    comptime!(StageEvent::RhsLoaded {
                        current: current_computation + 1,
                        total: acc_total_iterations
                    }),
                );

                let accumulator = acc.index_mut(acc_iter);
                TMM::execute(lhs_fragment, current, accumulator, config.to_tmm_config());
                SEL::on_event(
                    &mut listener,
                    comptime!(StageEvent::TmmCompleted {
                        current: current_computation,
                        total: acc_total_iterations
                    }),
                );

                comptime![acc_iter += 1];
            }

            let last = if comptime! {acc_iter % 2 == 0} {
                &mut rhs_fragments.0
            } else {
                &mut rhs_fragments.1
            };

            let accumulator = acc.index_mut(acc_iter);
            TMM::execute(lhs_fragment, last, accumulator, config.to_tmm_config());
            SEL::on_event(
                &mut listener,
                comptime!(StageEvent::TmmCompleted {
                    current: k_iter * num_accumulators + acc_iter,
                    total: acc_total_iterations
                }),
            );

            comptime![k_iter += 1];
        }

        SEL::on_event(&mut listener, comptime!(StageEvent::Finish));
    }
}
