use std::ops::Range;

use cubecl_core as cubecl;
use cubecl_core::prelude::*;

use crate::matmul::cmma::base::{Dimensions, DimensionsExpand, Offsets, OffsetsExpand};
use crate::matmul::cmma::config::CmmaBlockConfig;
use crate::matmul::tests::test_utils::{assert_equals_range, create_empty};
use crate::matmul::{
    cmma::{config::CmmaComptimeInfo, load_shared_memory::*},
    tests::test_utils::range_tensor,
};

use super::base::DimsTestCase;

#[cube(launch_unchecked)]
fn load_lhs_test<F: Float>(
    lhs_tensor: &Tensor<F>,
    lhs_sm_arr: &mut Array<F>,
    k_offset: UInt,
    m: UInt,
    k: UInt,
    n: UInt,
    config: Comptime<CmmaComptimeInfo>,
) {
    let block_size_m = Comptime::map(config, |c| c.block_size_m);
    let block_size_k = Comptime::map(config, |c| c.block_size_k);
    let sm_size = block_size_k * block_size_m;

    let offsets = Offsets {
        batch_lhs: UInt::new(0),
        batch_rhs: UInt::new(0),
        batch_out: UInt::new(0),
        cube_row: UInt::new(0),
        cube_col: UInt::new(0),
        k: k_offset,
    };

    let mut lhs_sm = SharedMemory::<F>::new(Comptime::get(sm_size));
    for i in range(0u32, Comptime::get(sm_size), Comptime::new(false)) {
        lhs_sm[i] = F::new(0.);
    }

    let dims = Dimensions { m, k, n };

    load_lhs(lhs_tensor, offsets, &mut lhs_sm, UInt::new(2), dims, config);

    for i in range(0u32, Comptime::get(sm_size), Comptime::new(false)) {
        lhs_sm_arr[i] = lhs_sm[i];
    }
}

#[cube(launch_unchecked)]
fn load_rhs_test<F: Float>(
    rhs_tensor: &Tensor<F>,
    rhs_sm_arr: &mut Array<F>,
    k_offset: UInt,
    m: UInt,
    k: UInt,
    n: UInt,
    config: Comptime<CmmaComptimeInfo>,
) {
    let block_size_k = Comptime::map(config, |c| c.block_size_k);
    let block_size_n = Comptime::map(config, |c| c.block_size_n);
    let sm_size = block_size_k * block_size_n;

    let offsets = Offsets {
        batch_lhs: UInt::new(0),
        batch_rhs: UInt::new(0),
        batch_out: UInt::new(0),
        cube_row: UInt::new(0),
        cube_col: UInt::new(0),
        k: k_offset,
    };

    let mut rhs_sm = SharedMemory::<F>::new(Comptime::get(sm_size));
    for i in range(0u32, Comptime::get(sm_size), Comptime::new(false)) {
        rhs_sm[i] = F::new(0.);
    }

    let dims = Dimensions { m, k, n };

    load_rhs(rhs_tensor, offsets, &mut rhs_sm, UInt::new(2), dims, config);

    for i in range(0u32, Comptime::get(sm_size), Comptime::new(false)) {
        rhs_sm_arr[i] = rhs_sm[i];
    }
}

enum InputTensor {
    Lhs,
    Rhs,
}

fn load_shared_memory_test_case<R: Runtime>(
    input: InputTensor,
    dims: DimsTestCase,
    k_offset: usize,
    config: CmmaBlockConfig,
    expected: &[f32],
    device: &R::Device,
    range: Range<usize>,
) {
    let client = R::client(device);

    for vectorization in [1, 2, 4] {
        let (tensor, sm, sm_size) = match input {
            InputTensor::Lhs => (
                range_tensor::<R>(&client, dims.m, dims.k),
                create_empty::<R>(&client, config.b_k, config.b_mn),
                config.b_k * config.b_mn,
            ),
            InputTensor::Rhs => (
                range_tensor::<R>(&client, dims.k, dims.n),
                create_empty::<R>(&client, config.b_k, config.b_mn),
                config.b_k * config.b_mn,
            ),
        };

        unsafe {
            load_lhs_test::launch_unchecked::<F32, R>(
                &R::client(device),
                config.cube_count::<R>(&[dims.m, dims.n]),
                config.cube_dim(),
                TensorArg::from_raw_parts(
                    &tensor.handle,
                    &tensor.strides,
                    &tensor.shape,
                    vectorization,
                ),
                ArrayArg::from_raw_parts(&sm, sm_size, 1),
                ScalarArg::new(k_offset as u32),
                ScalarArg::new(dims.m as u32),
                ScalarArg::new(dims.k as u32),
                ScalarArg::new(dims.n as u32),
                config.comptime_info(dims.m, dims.k, dims.n),
            );
        };

        assert_equals_range::<R>(&client, sm, expected, range.clone());
    }
}

/// Exported test
pub fn load_shared_memory_lhs_warp_test<R: Runtime>(device: &R::Device) {
    load_shared_memory_test_case::<R>(
        InputTensor::Lhs,
        DimsTestCase {
            m: 64,
            k: 32,
            n: 64,
        },
        0,
        CmmaBlockConfig::new(64, 32),
        &[
            0.0, 1.0, 2.0, 3.0, 4.0, 5.0, 6.0, 7.0, 8.0, 9.0, 10.0, 11.0, 12.0, 13.0, 14.0, 15.0,
            32.0, 33.0, 34.0, 35.0, 36.0, 37.0, 38.0, 39.0, 40.0, 41.0, 42.0, 43.0, 44.0, 45.0,
            46.0, 47.0, 64.0, 65.0, 66.0, 67.0, 68.0, 69.0, 70.0, 71.0, 72.0, 73.0, 74.0, 75.0,
            76.0, 77.0, 78.0, 79.0, 96.0, 97.0, 98.0, 99.0, 100.0, 101.0, 102.0, 103.0, 104.0,
            105.0, 106.0, 107.0, 108.0, 109.0, 110.0, 111.0, 128.0, 129.0, 130.0, 131.0, 132.0,
            133.0, 134.0, 135.0, 136.0, 137.0, 138.0, 139.0, 140.0, 141.0, 142.0, 143.0, 160.0,
            161.0, 162.0, 163.0, 164.0, 165.0, 166.0, 167.0, 168.0, 169.0, 170.0, 171.0, 172.0,
            173.0, 174.0, 175.0, 192.0, 193.0, 194.0, 195.0, 196.0, 197.0, 198.0, 199.0, 200.0,
            201.0, 202.0, 203.0, 204.0, 205.0, 206.0, 207.0, 224.0, 225.0, 226.0, 227.0, 228.0,
            229.0, 230.0, 231.0, 232.0, 233.0, 234.0, 235.0, 236.0, 237.0, 238.0, 239.0, 256.0,
            257.0, 258.0, 259.0, 260.0, 261.0, 262.0, 263.0, 264.0, 265.0, 266.0, 267.0, 268.0,
            269.0, 270.0, 271.0, 288.0, 289.0, 290.0, 291.0, 292.0, 293.0, 294.0, 295.0, 296.0,
            297.0, 298.0, 299.0, 300.0, 301.0, 302.0, 303.0, 320.0, 321.0, 322.0, 323.0, 324.0,
            325.0, 326.0, 327.0, 328.0, 329.0, 330.0, 331.0, 332.0, 333.0, 334.0, 335.0, 352.0,
            353.0, 354.0, 355.0, 356.0, 357.0, 358.0, 359.0, 360.0, 361.0, 362.0, 363.0, 364.0,
            365.0, 366.0, 367.0, 384.0, 385.0, 386.0, 387.0, 388.0, 389.0, 390.0, 391.0, 392.0,
            393.0, 394.0, 395.0, 396.0, 397.0, 398.0, 399.0, 416.0, 417.0, 418.0, 419.0, 420.0,
            421.0, 422.0, 423.0, 424.0, 425.0, 426.0, 427.0, 428.0, 429.0, 430.0, 431.0, 448.0,
            449.0, 450.0, 451.0, 452.0, 453.0, 454.0, 455.0, 456.0, 457.0, 458.0, 459.0, 460.0,
            461.0, 462.0, 463.0, 480.0, 481.0, 482.0, 483.0, 484.0, 485.0, 486.0, 487.0, 488.0,
            489.0, 490.0, 491.0, 492.0, 493.0, 494.0, 495.0,
        ],
        device,
        0..256,
    );
}

/// Exported test
pub fn load_shared_memory_rhs_warp_test<R: Runtime>(device: &R::Device) {
    load_shared_memory_test_case::<R>(
        InputTensor::Rhs,
        DimsTestCase {
            m: 64,
            k: 32,
            n: 64,
        },
        0,
        CmmaBlockConfig::new(64, 32),
        &[
            0.0, 1.0, 2.0, 3.0, 4.0, 5.0, 6.0, 7.0, 8.0, 9.0, 10.0, 11.0, 12.0, 13.0, 14.0, 15.0,
            32.0, 33.0, 34.0, 35.0, 36.0, 37.0, 38.0, 39.0, 40.0, 41.0, 42.0, 43.0, 44.0, 45.0,
            46.0, 47.0, 64.0, 65.0, 66.0, 67.0, 68.0, 69.0, 70.0, 71.0, 72.0, 73.0, 74.0, 75.0,
            76.0, 77.0, 78.0, 79.0, 96.0, 97.0, 98.0, 99.0, 100.0, 101.0, 102.0, 103.0, 104.0,
            105.0, 106.0, 107.0, 108.0, 109.0, 110.0, 111.0, 128.0, 129.0, 130.0, 131.0, 132.0,
            133.0, 134.0, 135.0, 136.0, 137.0, 138.0, 139.0, 140.0, 141.0, 142.0, 143.0, 160.0,
            161.0, 162.0, 163.0, 164.0, 165.0, 166.0, 167.0, 168.0, 169.0, 170.0, 171.0, 172.0,
            173.0, 174.0, 175.0, 192.0, 193.0, 194.0, 195.0, 196.0, 197.0, 198.0, 199.0, 200.0,
            201.0, 202.0, 203.0, 204.0, 205.0, 206.0, 207.0, 224.0, 225.0, 226.0, 227.0, 228.0,
            229.0, 230.0, 231.0, 232.0, 233.0, 234.0, 235.0, 236.0, 237.0, 238.0, 239.0, 256.0,
            257.0, 258.0, 259.0, 260.0, 261.0, 262.0, 263.0, 264.0, 265.0, 266.0, 267.0, 268.0,
            269.0, 270.0, 271.0, 288.0, 289.0, 290.0, 291.0, 292.0, 293.0, 294.0, 295.0, 296.0,
            297.0, 298.0, 299.0, 300.0, 301.0, 302.0, 303.0, 320.0, 321.0, 322.0, 323.0, 324.0,
            325.0, 326.0, 327.0, 328.0, 329.0, 330.0, 331.0, 332.0, 333.0, 334.0, 335.0, 352.0,
            353.0, 354.0, 355.0, 356.0, 357.0, 358.0, 359.0, 360.0, 361.0, 362.0, 363.0, 364.0,
            365.0, 366.0, 367.0, 384.0, 385.0, 386.0, 387.0, 388.0, 389.0, 390.0, 391.0, 392.0,
            393.0, 394.0, 395.0, 396.0, 397.0, 398.0, 399.0, 416.0, 417.0, 418.0, 419.0, 420.0,
            421.0, 422.0, 423.0, 424.0, 425.0, 426.0, 427.0, 428.0, 429.0, 430.0, 431.0, 448.0,
            449.0, 450.0, 451.0, 452.0, 453.0, 454.0, 455.0, 456.0, 457.0, 458.0, 459.0, 460.0,
            461.0, 462.0, 463.0, 480.0, 481.0, 482.0, 483.0, 484.0, 485.0, 486.0, 487.0, 488.0,
            489.0, 490.0, 491.0, 492.0, 493.0, 494.0, 495.0,
        ],
        device,
        0..256,
    );
}

/// Exported test
pub fn load_shared_memory_lhs_vertical_out_of_bound_warp_test<R: Runtime>(device: &R::Device) {
    load_shared_memory_test_case::<R>(
        InputTensor::Lhs,
        DimsTestCase {
            m: 12,
            k: 64,
            n: 64,
        },
        0,
        CmmaBlockConfig::new(64, 32),
        &[
            0.0, 1.0, 2.0, 3.0, 4.0, 5.0, 6.0, 7.0, 8.0, 9.0, 10.0, 11.0, 12.0, 13.0, 14.0, 15.0,
            64.0, 65.0, 66.0, 67.0, 68.0, 69.0, 70.0, 71.0, 72.0, 73.0, 74.0, 75.0, 76.0, 77.0,
            78.0, 79.0, 128.0, 129.0, 130.0, 131.0, 132.0, 133.0, 134.0, 135.0, 136.0, 137.0,
            138.0, 139.0, 140.0, 141.0, 142.0, 143.0, 192.0, 193.0, 194.0, 195.0, 196.0, 197.0,
            198.0, 199.0, 200.0, 201.0, 202.0, 203.0, 204.0, 205.0, 206.0, 207.0, 256.0, 257.0,
            258.0, 259.0, 260.0, 261.0, 262.0, 263.0, 264.0, 265.0, 266.0, 267.0, 268.0, 269.0,
            270.0, 271.0, 320.0, 321.0, 322.0, 323.0, 324.0, 325.0, 326.0, 327.0, 328.0, 329.0,
            330.0, 331.0, 332.0, 333.0, 334.0, 335.0, 384.0, 385.0, 386.0, 387.0, 388.0, 389.0,
            390.0, 391.0, 392.0, 393.0, 394.0, 395.0, 396.0, 397.0, 398.0, 399.0, 448.0, 449.0,
            450.0, 451.0, 452.0, 453.0, 454.0, 455.0, 456.0, 457.0, 458.0, 459.0, 460.0, 461.0,
            462.0, 463.0, 512.0, 513.0, 514.0, 515.0, 516.0, 517.0, 518.0, 519.0, 520.0, 521.0,
            522.0, 523.0, 524.0, 525.0, 526.0, 527.0, 576.0, 577.0, 578.0, 579.0, 580.0, 581.0,
            582.0, 583.0, 584.0, 585.0, 586.0, 587.0, 588.0, 589.0, 590.0, 591.0, 640.0, 641.0,
            642.0, 643.0, 644.0, 645.0, 646.0, 647.0, 648.0, 649.0, 650.0, 651.0, 652.0, 653.0,
            654.0, 655.0, 704.0, 705.0, 706.0, 707.0, 708.0, 709.0, 710.0, 711.0, 712.0, 713.0,
            714.0, 715.0, 716.0, 717.0, 718.0, 719.0, 0.0, 0.0, 0.0, 0.0, 0.0, 0.0, 0.0, 0.0, 0.0,
            0.0, 0.0, 0.0, 0.0, 0.0, 0.0, 0.0, 0.0, 0.0, 0.0, 0.0, 0.0, 0.0, 0.0, 0.0, 0.0, 0.0,
            0.0, 0.0, 0.0, 0.0, 0.0, 0.0, 0.0, 0.0, 0.0, 0.0, 0.0, 0.0, 0.0, 0.0, 0.0, 0.0, 0.0,
            0.0, 0.0, 0.0, 0.0, 0.0, 0.0, 0.0, 0.0, 0.0, 0.0, 0.0, 0.0, 0.0, 0.0, 0.0, 0.0, 0.0,
            0.0, 0.0, 0.0, 0.0,
        ],
        device,
        0..256,
    );
}

/// Exported test
pub fn load_shared_memory_lhs_horizontal_out_of_bound_warp_test<R: Runtime>(device: &R::Device) {
    load_shared_memory_test_case::<R>(
        InputTensor::Lhs,
        DimsTestCase {
            m: 64,
            k: 12,
            n: 64,
        },
        0,
        CmmaBlockConfig::new(64, 32),
        &[
            0.0, 1.0, 2.0, 3.0, 4.0, 5.0, 6.0, 7.0, 8.0, 9.0, 10.0, 11.0, 0.0, 0.0, 0.0, 0.0, 12.0,
            13.0, 14.0, 15.0, 16.0, 17.0, 18.0, 19.0, 20.0, 21.0, 22.0, 23.0, 0.0, 0.0, 0.0, 0.0,
            24.0, 25.0, 26.0, 27.0, 28.0, 29.0, 30.0, 31.0, 32.0, 33.0, 34.0, 35.0, 0.0, 0.0, 0.0,
            0.0, 36.0, 37.0, 38.0, 39.0, 40.0, 41.0, 42.0, 43.0, 44.0, 45.0, 46.0, 47.0, 0.0, 0.0,
            0.0, 0.0, 48.0, 49.0, 50.0, 51.0, 52.0, 53.0, 54.0, 55.0, 56.0, 57.0, 58.0, 59.0, 0.0,
            0.0, 0.0, 0.0, 60.0, 61.0, 62.0, 63.0, 64.0, 65.0, 66.0, 67.0, 68.0, 69.0, 70.0, 71.0,
            0.0, 0.0, 0.0, 0.0, 72.0, 73.0, 74.0, 75.0, 76.0, 77.0, 78.0, 79.0, 80.0, 81.0, 82.0,
            83.0, 0.0, 0.0, 0.0, 0.0, 84.0, 85.0, 86.0, 87.0, 88.0, 89.0, 90.0, 91.0, 92.0, 93.0,
            94.0, 95.0, 0.0, 0.0, 0.0, 0.0, 96.0, 97.0, 98.0, 99.0, 100.0, 101.0, 102.0, 103.0,
            104.0, 105.0, 106.0, 107.0, 0.0, 0.0, 0.0, 0.0, 108.0, 109.0, 110.0, 111.0, 112.0,
            113.0, 114.0, 115.0, 116.0, 117.0, 118.0, 119.0, 0.0, 0.0, 0.0, 0.0, 120.0, 121.0,
            122.0, 123.0, 124.0, 125.0, 126.0, 127.0, 128.0, 129.0, 130.0, 131.0, 0.0, 0.0, 0.0,
            0.0, 132.0, 133.0, 134.0, 135.0, 136.0, 137.0, 138.0, 139.0, 140.0, 141.0, 142.0,
            143.0, 0.0, 0.0, 0.0, 0.0, 144.0, 145.0, 146.0, 147.0, 148.0, 149.0, 150.0, 151.0,
            152.0, 153.0, 154.0, 155.0, 0.0, 0.0, 0.0, 0.0, 156.0, 157.0, 158.0, 159.0, 160.0,
            161.0, 162.0, 163.0, 164.0, 165.0, 166.0, 167.0, 0.0, 0.0, 0.0, 0.0, 168.0, 169.0,
            170.0, 171.0, 172.0, 173.0, 174.0, 175.0, 176.0, 177.0, 178.0, 179.0, 0.0, 0.0, 0.0,
            0.0, 180.0, 181.0, 182.0, 183.0, 184.0, 185.0, 186.0, 187.0, 188.0, 189.0, 190.0,
            191.0, 0.0, 0.0, 0.0, 0.0,
        ],
        device,
        0..256,
    );
}

/// Exported test
pub fn load_shared_memory_lhs_whole_out_of_bound_warp_test<R: Runtime>(device: &R::Device) {
    load_shared_memory_test_case::<R>(
        InputTensor::Lhs,
        DimsTestCase {
            m: 12,
            k: 12,
            n: 64,
        },
        0,
        CmmaBlockConfig::new(64, 32),
        &[
            0.0, 1.0, 2.0, 3.0, 4.0, 5.0, 6.0, 7.0, 8.0, 9.0, 10.0, 11.0, 0.0, 0.0, 0.0, 0.0, 12.0,
            13.0, 14.0, 15.0, 16.0, 17.0, 18.0, 19.0, 20.0, 21.0, 22.0, 23.0, 0.0, 0.0, 0.0, 0.0,
            24.0, 25.0, 26.0, 27.0, 28.0, 29.0, 30.0, 31.0, 32.0, 33.0, 34.0, 35.0, 0.0, 0.0, 0.0,
            0.0, 36.0, 37.0, 38.0, 39.0, 40.0, 41.0, 42.0, 43.0, 44.0, 45.0, 46.0, 47.0, 0.0, 0.0,
            0.0, 0.0, 48.0, 49.0, 50.0, 51.0, 52.0, 53.0, 54.0, 55.0, 56.0, 57.0, 58.0, 59.0, 0.0,
            0.0, 0.0, 0.0, 60.0, 61.0, 62.0, 63.0, 64.0, 65.0, 66.0, 67.0, 68.0, 69.0, 70.0, 71.0,
            0.0, 0.0, 0.0, 0.0, 72.0, 73.0, 74.0, 75.0, 76.0, 77.0, 78.0, 79.0, 80.0, 81.0, 82.0,
            83.0, 0.0, 0.0, 0.0, 0.0, 84.0, 85.0, 86.0, 87.0, 88.0, 89.0, 90.0, 91.0, 92.0, 93.0,
            94.0, 95.0, 0.0, 0.0, 0.0, 0.0, 96.0, 97.0, 98.0, 99.0, 100.0, 101.0, 102.0, 103.0,
            104.0, 105.0, 106.0, 107.0, 0.0, 0.0, 0.0, 0.0, 108.0, 109.0, 110.0, 111.0, 112.0,
            113.0, 114.0, 115.0, 116.0, 117.0, 118.0, 119.0, 0.0, 0.0, 0.0, 0.0, 120.0, 121.0,
            122.0, 123.0, 124.0, 125.0, 126.0, 127.0, 128.0, 129.0, 130.0, 131.0, 0.0, 0.0, 0.0,
            0.0, 132.0, 133.0, 134.0, 135.0, 136.0, 137.0, 138.0, 139.0, 140.0, 141.0, 142.0,
            143.0, 0.0, 0.0, 0.0, 0.0, 0.0, 0.0, 0.0, 0.0, 0.0, 0.0, 0.0, 0.0, 0.0, 0.0, 0.0, 0.0,
            0.0, 0.0, 0.0, 0.0, 0.0, 0.0, 0.0, 0.0, 0.0, 0.0, 0.0, 0.0, 0.0, 0.0, 0.0, 0.0, 0.0,
            0.0, 0.0, 0.0, 0.0, 0.0, 0.0, 0.0, 0.0, 0.0, 0.0, 0.0, 0.0, 0.0, 0.0, 0.0, 0.0, 0.0,
            0.0, 0.0, 0.0, 0.0, 0.0, 0.0, 0.0, 0.0, 0.0, 0.0, 0.0, 0.0, 0.0, 0.0, 0.0, 0.0, 0.0,
            0.0,
        ],
        device,
        0..256,
    );
}

/// Exported test
pub fn load_shared_memory_lhs_second_warp_test<R: Runtime>(device: &R::Device) {
    load_shared_memory_test_case::<R>(
        InputTensor::Lhs,
        DimsTestCase {
            m: 64,
            k: 64,
            n: 64,
        },
        0,
        CmmaBlockConfig::new(64, 32),
        &[
            16., 17., 18., 19., 20., 21., 22., 23., 24., 25., 26., 27., 28., 29., 30., 31., 80.,
            81., 82., 83., 84., 85., 86., 87., 88., 89., 90., 91., 92., 93., 94., 95., 144., 145.,
            146., 147., 148., 149., 150., 151., 152., 153., 154., 155., 156., 157., 158., 159.,
            208., 209., 210., 211., 212., 213., 214., 215., 216., 217., 218., 219., 220., 221.,
            222., 223., 272., 273., 274., 275., 276., 277., 278., 279., 280., 281., 282., 283.,
            284., 285., 286., 287., 336., 337., 338., 339., 340., 341., 342., 343., 344., 345.,
            346., 347., 348., 349., 350., 351., 400., 401., 402., 403., 404., 405., 406., 407.,
            408., 409., 410., 411., 412., 413., 414., 415., 464., 465., 466., 467., 468., 469.,
            470., 471., 472., 473., 474., 475., 476., 477., 478., 479., 528., 529., 530., 531.,
            532., 533., 534., 535., 536., 537., 538., 539., 540., 541., 542., 543., 592., 593.,
            594., 595., 596., 597., 598., 599., 600., 601., 602., 603., 604., 605., 606., 607.,
            656., 657., 658., 659., 660., 661., 662., 663., 664., 665., 666., 667., 668., 669.,
            670., 671., 720., 721., 722., 723., 724., 725., 726., 727., 728., 729., 730., 731.,
            732., 733., 734., 735., 784., 785., 786., 787., 788., 789., 790., 791., 792., 793.,
            794., 795., 796., 797., 798., 799., 848., 849., 850., 851., 852., 853., 854., 855.,
            856., 857., 858., 859., 860., 861., 862., 863., 912., 913., 914., 915., 916., 917.,
            918., 919., 920., 921., 922., 923., 924., 925., 926., 927., 976., 977., 978., 979.,
            980., 981., 982., 983., 984., 985., 986., 987., 988., 989., 990., 991.,
        ],
        device,
        256..512,
    );
}

/// Exported test
pub fn load_shared_memory_rhs_second_warp_test<R: Runtime>(device: &R::Device) {
    load_shared_memory_test_case::<R>(
        InputTensor::Rhs,
        DimsTestCase {
            m: 64,
            k: 64,
            n: 64,
        },
        0,
        CmmaBlockConfig::new(64, 32),
        &[
            16.0, 17.0, 18.0, 19.0, 20.0, 21.0, 22.0, 23.0, 24.0, 25.0, 26.0, 27.0, 28.0, 29.0,
            30.0, 31.0, 80.0, 81.0, 82.0, 83.0, 84.0, 85.0, 86.0, 87.0, 88.0, 89.0, 90.0, 91.0,
            92.0, 93.0, 94.0, 95.0, 144.0, 145.0, 146.0, 147.0, 148.0, 149.0, 150.0, 151.0, 152.0,
            153.0, 154.0, 155.0, 156.0, 157.0, 158.0, 159.0, 208.0, 209.0, 210.0, 211.0, 212.0,
            213.0, 214.0, 215.0, 216.0, 217.0, 218.0, 219.0, 220.0, 221.0, 222.0, 223.0, 272.0,
            273.0, 274.0, 275.0, 276.0, 277.0, 278.0, 279.0, 280.0, 281.0, 282.0, 283.0, 284.0,
            285.0, 286.0, 287.0, 336.0, 337.0, 338.0, 339.0, 340.0, 341.0, 342.0, 343.0, 344.0,
            345.0, 346.0, 347.0, 348.0, 349.0, 350.0, 351.0, 400.0, 401.0, 402.0, 403.0, 404.0,
            405.0, 406.0, 407.0, 408.0, 409.0, 410.0, 411.0, 412.0, 413.0, 414.0, 415.0, 464.0,
            465.0, 466.0, 467.0, 468.0, 469.0, 470.0, 471.0, 472.0, 473.0, 474.0, 475.0, 476.0,
            477.0, 478.0, 479.0, 528.0, 529.0, 530.0, 531.0, 532.0, 533.0, 534.0, 535.0, 536.0,
            537.0, 538.0, 539.0, 540.0, 541.0, 542.0, 543.0, 592.0, 593.0, 594.0, 595.0, 596.0,
            597.0, 598.0, 599.0, 600.0, 601.0, 602.0, 603.0, 604.0, 605.0, 606.0, 607.0, 656.0,
            657.0, 658.0, 659.0, 660.0, 661.0, 662.0, 663.0, 664.0, 665.0, 666.0, 667.0, 668.0,
            669.0, 670.0, 671.0, 720.0, 721.0, 722.0, 723.0, 724.0, 725.0, 726.0, 727.0, 728.0,
            729.0, 730.0, 731.0, 732.0, 733.0, 734.0, 735.0, 784.0, 785.0, 786.0, 787.0, 788.0,
            789.0, 790.0, 791.0, 792.0, 793.0, 794.0, 795.0, 796.0, 797.0, 798.0, 799.0, 848.0,
            849.0, 850.0, 851.0, 852.0, 853.0, 854.0, 855.0, 856.0, 857.0, 858.0, 859.0, 860.0,
            861.0, 862.0, 863.0, 912.0, 913.0, 914.0, 915.0, 916.0, 917.0, 918.0, 919.0, 920.0,
            921.0, 922.0, 923.0, 924.0, 925.0, 926.0, 927.0, 976.0, 977.0, 978.0, 979.0, 980.0,
            981.0, 982.0, 983.0, 984.0, 985.0, 986.0, 987.0, 988.0, 989.0, 990.0, 991.0,
        ],
        device,
        256..512,
    );
}

/// Exported test
pub fn load_shared_memory_lhs_third_warp_test<R: Runtime>(device: &R::Device) {
    load_shared_memory_test_case::<R>(
        InputTensor::Lhs,
        DimsTestCase {
            m: 64,
            k: 64,
            n: 64,
        },
        0,
        CmmaBlockConfig::new(64, 32),
        &[
            16.0, 17.0, 18.0, 19.0, 20.0, 21.0, 22.0, 23.0, 24.0, 25.0, 26.0, 27.0, 28.0, 29.0,
            30.0, 31.0, 80.0, 81.0, 82.0, 83.0, 84.0, 85.0, 86.0, 87.0, 88.0, 89.0, 90.0, 91.0,
            92.0, 93.0, 94.0, 95.0, 144.0, 145.0, 146.0, 147.0, 148.0, 149.0, 150.0, 151.0, 152.0,
            153.0, 154.0, 155.0, 156.0, 157.0, 158.0, 159.0, 208.0, 209.0, 210.0, 211.0, 212.0,
            213.0, 214.0, 215.0, 216.0, 217.0, 218.0, 219.0, 220.0, 221.0, 222.0, 223.0, 272.0,
            273.0, 274.0, 275.0, 276.0, 277.0, 278.0, 279.0, 280.0, 281.0, 282.0, 283.0, 284.0,
            285.0, 286.0, 287.0, 336.0, 337.0, 338.0, 339.0, 340.0, 341.0, 342.0, 343.0, 344.0,
            345.0, 346.0, 347.0, 348.0, 349.0, 350.0, 351.0, 400.0, 401.0, 402.0, 403.0, 404.0,
            405.0, 406.0, 407.0, 408.0, 409.0, 410.0, 411.0, 412.0, 413.0, 414.0, 415.0, 464.0,
            465.0, 466.0, 467.0, 468.0, 469.0, 470.0, 471.0, 472.0, 473.0, 474.0, 475.0, 476.0,
            477.0, 478.0, 479.0, 528.0, 529.0, 530.0, 531.0, 532.0, 533.0, 534.0, 535.0, 536.0,
            537.0, 538.0, 539.0, 540.0, 541.0, 542.0, 543.0, 592.0, 593.0, 594.0, 595.0, 596.0,
            597.0, 598.0, 599.0, 600.0, 601.0, 602.0, 603.0, 604.0, 605.0, 606.0, 607.0, 656.0,
            657.0, 658.0, 659.0, 660.0, 661.0, 662.0, 663.0, 664.0, 665.0, 666.0, 667.0, 668.0,
            669.0, 670.0, 671.0, 720.0, 721.0, 722.0, 723.0, 724.0, 725.0, 726.0, 727.0, 728.0,
            729.0, 730.0, 731.0, 732.0, 733.0, 734.0, 735.0, 784.0, 785.0, 786.0, 787.0, 788.0,
            789.0, 790.0, 791.0, 792.0, 793.0, 794.0, 795.0, 796.0, 797.0, 798.0, 799.0, 848.0,
            849.0, 850.0, 851.0, 852.0, 853.0, 854.0, 855.0, 856.0, 857.0, 858.0, 859.0, 860.0,
            861.0, 862.0, 863.0, 912.0, 913.0, 914.0, 915.0, 916.0, 917.0, 918.0, 919.0, 920.0,
            921.0, 922.0, 923.0, 924.0, 925.0, 926.0, 927.0, 976.0, 977.0, 978.0, 979.0, 980.0,
            981.0, 982.0, 983.0, 984.0, 985.0, 986.0, 987.0, 988.0, 989.0, 990.0, 991.0,
        ],
        device,
        256..512,
    );
}

/// Exported test
pub fn load_shared_memory_rhs_third_warp_test<R: Runtime>(device: &R::Device) {
    load_shared_memory_test_case::<R>(
        InputTensor::Rhs,
        DimsTestCase {
            m: 64,
            k: 64,
            n: 64,
        },
        0,
        CmmaBlockConfig::new(64, 32),
        &[
            1024.0, 1025.0, 1026.0, 1027.0, 1028.0, 1029.0, 1030.0, 1031.0, 1032.0, 1033.0, 1034.0,
            1035.0, 1036.0, 1037.0, 1038.0, 1039.0, 1088.0, 1089.0, 1090.0, 1091.0, 1092.0, 1093.0,
            1094.0, 1095.0, 1096.0, 1097.0, 1098.0, 1099.0, 1100.0, 1101.0, 1102.0, 1103.0, 1152.0,
            1153.0, 1154.0, 1155.0, 1156.0, 1157.0, 1158.0, 1159.0, 1160.0, 1161.0, 1162.0, 1163.0,
            1164.0, 1165.0, 1166.0, 1167.0, 1216.0, 1217.0, 1218.0, 1219.0, 1220.0, 1221.0, 1222.0,
            1223.0, 1224.0, 1225.0, 1226.0, 1227.0, 1228.0, 1229.0, 1230.0, 1231.0, 1280.0, 1281.0,
            1282.0, 1283.0, 1284.0, 1285.0, 1286.0, 1287.0, 1288.0, 1289.0, 1290.0, 1291.0, 1292.0,
            1293.0, 1294.0, 1295.0, 1344.0, 1345.0, 1346.0, 1347.0, 1348.0, 1349.0, 1350.0, 1351.0,
            1352.0, 1353.0, 1354.0, 1355.0, 1356.0, 1357.0, 1358.0, 1359.0, 1408.0, 1409.0, 1410.0,
            1411.0, 1412.0, 1413.0, 1414.0, 1415.0, 1416.0, 1417.0, 1418.0, 1419.0, 1420.0, 1421.0,
            1422.0, 1423.0, 1472.0, 1473.0, 1474.0, 1475.0, 1476.0, 1477.0, 1478.0, 1479.0, 1480.0,
            1481.0, 1482.0, 1483.0, 1484.0, 1485.0, 1486.0, 1487.0, 1536.0, 1537.0, 1538.0, 1539.0,
            1540.0, 1541.0, 1542.0, 1543.0, 1544.0, 1545.0, 1546.0, 1547.0, 1548.0, 1549.0, 1550.0,
            1551.0, 1600.0, 1601.0, 1602.0, 1603.0, 1604.0, 1605.0, 1606.0, 1607.0, 1608.0, 1609.0,
            1610.0, 1611.0, 1612.0, 1613.0, 1614.0, 1615.0, 1664.0, 1665.0, 1666.0, 1667.0, 1668.0,
            1669.0, 1670.0, 1671.0, 1672.0, 1673.0, 1674.0, 1675.0, 1676.0, 1677.0, 1678.0, 1679.0,
            1728.0, 1729.0, 1730.0, 1731.0, 1732.0, 1733.0, 1734.0, 1735.0, 1736.0, 1737.0, 1738.0,
            1739.0, 1740.0, 1741.0, 1742.0, 1743.0, 1792.0, 1793.0, 1794.0, 1795.0, 1796.0, 1797.0,
            1798.0, 1799.0, 1800.0, 1801.0, 1802.0, 1803.0, 1804.0, 1805.0, 1806.0, 1807.0, 1856.0,
            1857.0, 1858.0, 1859.0, 1860.0, 1861.0, 1862.0, 1863.0, 1864.0, 1865.0, 1866.0, 1867.0,
            1868.0, 1869.0, 1870.0, 1871.0, 1920.0, 1921.0, 1922.0, 1923.0, 1924.0, 1925.0, 1926.0,
            1927.0, 1928.0, 1929.0, 1930.0, 1931.0, 1932.0, 1933.0, 1934.0, 1935.0, 1984.0, 1985.0,
            1986.0, 1987.0, 1988.0, 1989.0, 1990.0, 1991.0, 1992.0, 1993.0, 1994.0, 1995.0, 1996.0,
            1997.0, 1998.0, 1999.0,
        ],
        device,
        512..768,
    );
}

/// Exported test
pub fn load_shared_memory_lhs_k_offset_test<R: Runtime>(device: &R::Device) {
    load_shared_memory_test_case::<R>(
        InputTensor::Lhs,
        DimsTestCase {
            m: 64,
            k: 64,
            n: 64,
        },
        32,
        CmmaBlockConfig::new(64, 32),
        &[
            32.0, 33.0, 34.0, 35.0, 36.0, 37.0, 38.0, 39.0, 40.0, 41.0, 42.0, 43.0, 44.0, 45.0,
            46.0, 47.0, 96.0, 97.0, 98.0, 99.0, 100.0, 101.0, 102.0, 103.0, 104.0, 105.0, 106.0,
            107.0, 108.0, 109.0, 110.0, 111.0, 160.0, 161.0, 162.0, 163.0, 164.0, 165.0, 166.0,
            167.0, 168.0, 169.0, 170.0, 171.0, 172.0, 173.0, 174.0, 175.0, 224.0, 225.0, 226.0,
            227.0, 228.0, 229.0, 230.0, 231.0, 232.0, 233.0, 234.0, 235.0, 236.0, 237.0, 238.0,
            239.0, 288.0, 289.0, 290.0, 291.0, 292.0, 293.0, 294.0, 295.0, 296.0, 297.0, 298.0,
            299.0, 300.0, 301.0, 302.0, 303.0, 352.0, 353.0, 354.0, 355.0, 356.0, 357.0, 358.0,
            359.0, 360.0, 361.0, 362.0, 363.0, 364.0, 365.0, 366.0, 367.0, 416.0, 417.0, 418.0,
            419.0, 420.0, 421.0, 422.0, 423.0, 424.0, 425.0, 426.0, 427.0, 428.0, 429.0, 430.0,
            431.0, 480.0, 481.0, 482.0, 483.0, 484.0, 485.0, 486.0, 487.0, 488.0, 489.0, 490.0,
            491.0, 492.0, 493.0, 494.0, 495.0, 544.0, 545.0, 546.0, 547.0, 548.0, 549.0, 550.0,
            551.0, 552.0, 553.0, 554.0, 555.0, 556.0, 557.0, 558.0, 559.0, 608.0, 609.0, 610.0,
            611.0, 612.0, 613.0, 614.0, 615.0, 616.0, 617.0, 618.0, 619.0, 620.0, 621.0, 622.0,
            623.0, 672.0, 673.0, 674.0, 675.0, 676.0, 677.0, 678.0, 679.0, 680.0, 681.0, 682.0,
            683.0, 684.0, 685.0, 686.0, 687.0, 736.0, 737.0, 738.0, 739.0, 740.0, 741.0, 742.0,
            743.0, 744.0, 745.0, 746.0, 747.0, 748.0, 749.0, 750.0, 751.0, 800.0, 801.0, 802.0,
            803.0, 804.0, 805.0, 806.0, 807.0, 808.0, 809.0, 810.0, 811.0, 812.0, 813.0, 814.0,
            815.0, 864.0, 865.0, 866.0, 867.0, 868.0, 869.0, 870.0, 871.0, 872.0, 873.0, 874.0,
            875.0, 876.0, 877.0, 878.0, 879.0, 928.0, 929.0, 930.0, 931.0, 932.0, 933.0, 934.0,
            935.0, 936.0, 937.0, 938.0, 939.0, 940.0, 941.0, 942.0, 943.0, 992.0, 993.0, 994.0,
            995.0, 996.0, 997.0, 998.0, 999.0, 1000.0, 1001.0, 1002.0, 1003.0, 1004.0, 1005.0,
            1006.0, 1007.0,
        ],
        device,
        0..256,
    );
}

/// Exported test
pub fn load_shared_memory_rhs_k_offset_test<R: Runtime>(device: &R::Device) {
    load_shared_memory_test_case::<R>(
        InputTensor::Rhs,
        DimsTestCase {
            m: 64,
            k: 64,
            n: 64,
        },
        32,
        CmmaBlockConfig::new(64, 32),
        &[
            32.0, 33.0, 34.0, 35.0, 36.0, 37.0, 38.0, 39.0, 40.0, 41.0, 42.0, 43.0, 44.0, 45.0,
            46.0, 47.0, 96.0, 97.0, 98.0, 99.0, 100.0, 101.0, 102.0, 103.0, 104.0, 105.0, 106.0,
            107.0, 108.0, 109.0, 110.0, 111.0, 160.0, 161.0, 162.0, 163.0, 164.0, 165.0, 166.0,
            167.0, 168.0, 169.0, 170.0, 171.0, 172.0, 173.0, 174.0, 175.0, 224.0, 225.0, 226.0,
            227.0, 228.0, 229.0, 230.0, 231.0, 232.0, 233.0, 234.0, 235.0, 236.0, 237.0, 238.0,
            239.0, 288.0, 289.0, 290.0, 291.0, 292.0, 293.0, 294.0, 295.0, 296.0, 297.0, 298.0,
            299.0, 300.0, 301.0, 302.0, 303.0, 352.0, 353.0, 354.0, 355.0, 356.0, 357.0, 358.0,
            359.0, 360.0, 361.0, 362.0, 363.0, 364.0, 365.0, 366.0, 367.0, 416.0, 417.0, 418.0,
            419.0, 420.0, 421.0, 422.0, 423.0, 424.0, 425.0, 426.0, 427.0, 428.0, 429.0, 430.0,
            431.0, 480.0, 481.0, 482.0, 483.0, 484.0, 485.0, 486.0, 487.0, 488.0, 489.0, 490.0,
            491.0, 492.0, 493.0, 494.0, 495.0, 544.0, 545.0, 546.0, 547.0, 548.0, 549.0, 550.0,
            551.0, 552.0, 553.0, 554.0, 555.0, 556.0, 557.0, 558.0, 559.0, 608.0, 609.0, 610.0,
            611.0, 612.0, 613.0, 614.0, 615.0, 616.0, 617.0, 618.0, 619.0, 620.0, 621.0, 622.0,
            623.0, 672.0, 673.0, 674.0, 675.0, 676.0, 677.0, 678.0, 679.0, 680.0, 681.0, 682.0,
            683.0, 684.0, 685.0, 686.0, 687.0, 736.0, 737.0, 738.0, 739.0, 740.0, 741.0, 742.0,
            743.0, 744.0, 745.0, 746.0, 747.0, 748.0, 749.0, 750.0, 751.0, 800.0, 801.0, 802.0,
            803.0, 804.0, 805.0, 806.0, 807.0, 808.0, 809.0, 810.0, 811.0, 812.0, 813.0, 814.0,
            815.0, 864.0, 865.0, 866.0, 867.0, 868.0, 869.0, 870.0, 871.0, 872.0, 873.0, 874.0,
            875.0, 876.0, 877.0, 878.0, 879.0, 928.0, 929.0, 930.0, 931.0, 932.0, 933.0, 934.0,
            935.0, 936.0, 937.0, 938.0, 939.0, 940.0, 941.0, 942.0, 943.0, 992.0, 993.0, 994.0,
            995.0, 996.0, 997.0, 998.0, 999.0, 1000.0, 1001.0, 1002.0, 1003.0, 1004.0, 1005.0,
            1006.0, 1007.0,
        ],
        device,
        0..256,
    );
}
