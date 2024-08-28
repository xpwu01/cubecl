use std::cmp::max;

use cubecl_core::{
    client::ComputeClient,
    frontend::{Float, TensorArg, TensorHandleRef, F16},
    ir::{Elem, FloatKind},
    Compiler, CubeDim, Feature, Runtime,
};

use crate::{
    matmul::cmma::{
        base::cmma_kernel,
        config::{cmma_cube_count, CmmaConfig, CmmaLaunchConfig},
    },
    tensor::{into_contiguous, matrix_layout, MatrixLayout, TensorHandle},
};

/// Matrix multiplication using [cooperative matrix-multiply and accumulate operations](cubecl_core::cmma).
pub fn matmul_cmma<R: Runtime, F: Float>(
    client: &ComputeClient<R::Server, R::Channel>,
    lhs: TensorHandle<R, F>,
    rhs: TensorHandle<R, F>,
    out: TensorHandle<R, F>,
) -> TensorHandle<R, F> {
    matmul_cmma_ref::<R, F>(client, lhs.as_ref(), rhs.as_ref(), out.as_ref());
    out
}

#[derive(Debug)]
pub enum UnavailabilityReason {
    HighlyPermutatedInput,
    ShapeMemoryLimitBusted,
    InvalidConfig(String),
    CmmaInstructionsUnsupported,
}

/// Checks if the matmul cmma can be used.
pub fn check_cmma_availability<R: Runtime>(
    client: &ComputeClient<R::Server, R::Channel>,
    config: Option<&CmmaLaunchConfig>,
) -> Result<(), UnavailabilityReason> {
    if !client.features().enabled(Feature::Cmma {
        a: Elem::Float(FloatKind::F16),
        b: Elem::Float(FloatKind::F16),
        c: Elem::Float(FloatKind::F32),
        m: 16,
        k: 16,
        n: 16,
    }) {
        return Err(UnavailabilityReason::CmmaInstructionsUnsupported);
    }

    if let Some(config) = config {
        let (b_m, b_k, b_n) = (
            config.block_size_m,
            config.block_size_k,
            config.block_size_n,
        );

        if b_k * max(b_m, b_n) > <R::Compiler as Compiler>::max_shared_memory_size() {
            return Err(UnavailabilityReason::ShapeMemoryLimitBusted);
        }

        if b_m * b_n > <R::Compiler as Compiler>::max_shared_memory_size() {
            return Err(UnavailabilityReason::ShapeMemoryLimitBusted);
        }
    }

    Ok(())
}
/// Matrix multiplication using [cooperative matrix-multiply and accumulate operations](cubecl_core::cmma).
pub fn matmul_cmma_ref<R: Runtime, F: Float>(
    client: &ComputeClient<R::Server, R::Channel>,
    lhs: TensorHandleRef<'_, R>,
    rhs: TensorHandleRef<'_, R>,
    out: TensorHandleRef<'_, R>,
) {
    let check_layout = |tensor: &TensorHandleRef<'_, R>| match matrix_layout(tensor.strides) {
        MatrixLayout::Contiguous => true,
        MatrixLayout::MildlyPermuted {
            transposed: _,
            batch_swap: _,
        } => false,
        MatrixLayout::HighlyPermuted => false,
    };

    let lhs_correct_layout = check_layout(&lhs);
    let rhs_correct_layout = check_layout(&rhs);

    match (lhs_correct_layout, rhs_correct_layout) {
        (true, true) => matmul_cmma_ref_no_check::<R, F>(client, lhs, rhs, out),
        (true, false) => matmul_cmma_ref_no_check::<R, F>(
            client,
            lhs,
            into_contiguous::<R, F>(client, rhs).as_ref(),
            out,
        ),
        (false, true) => matmul_cmma_ref_no_check::<R, F>(
            client,
            into_contiguous::<R, F>(client, lhs).as_ref(),
            rhs,
            out,
        ),
        (false, false) => matmul_cmma_ref_no_check::<R, F>(
            client,
            into_contiguous::<R, F>(client, lhs).as_ref(),
            into_contiguous::<R, F>(client, rhs).as_ref(),
            out,
        ),
    }
}

fn matmul_cmma_ref_no_check<R: Runtime, F: Float>(
    client: &ComputeClient<R::Server, R::Channel>,
    lhs: TensorHandleRef<'_, R>,
    rhs: TensorHandleRef<'_, R>,
    out: TensorHandleRef<'_, R>,
) {
    let rank = lhs.strides.len();

    let m = lhs.shape[rank - 2];
    let k = lhs.shape[rank - 1];
    let n = rhs.shape[rank - 1];

    let vectorization = |shape: usize| {
        [4, 2]
            .into_iter()
            .filter(|v| shape % v == 0)
            .map(|v| v as u8)
            .next()
            .unwrap_or(1)
    };

    let lhs_vectorization = vectorization(k);
    let rhs_vectorization = vectorization(n);
    let out_vectorization = vectorization(n);

    let launch_config = CmmaLaunchConfig {
        block_size_m: 64,
        block_size_k: 32,
        block_size_n: 64,
        unroll: true,
    };
    let cube_count = cmma_cube_count::<R>(out.shape, &launch_config);
    let cube_dim = CubeDim::new(16, 16, 1);

    unsafe {
        cmma_kernel::launch_unchecked::<F, F16, R>(
            client,
            cube_count,
            cube_dim,
            TensorArg::from_raw_parts(lhs.handle, lhs.strides, lhs.shape, lhs_vectorization),
            TensorArg::from_raw_parts(rhs.handle, rhs.strides, rhs.shape, rhs_vectorization),
            TensorArg::from_raw_parts(out.handle, out.strides, out.shape, out_vectorization),
            CmmaConfig::new(m, k, n, cube_dim, launch_config),
        );
    }
}
