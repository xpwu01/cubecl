use cubecl::prelude::*;
use cubecl_core::{self as cubecl, unexpanded};
use std::{marker::PhantomData, sync::Arc};

/// The read tag for [virtual tensor](VirtualTensor).
#[derive(Clone)]
pub struct Read;

/// The read write tag for [virtual tensor](VirtualTensor).
#[derive(Clone)]
pub struct ReadWrite;

/// Tensor representation that is decoupled from how the tensor is stored.
#[derive(Clone)]
pub struct VirtualTensor<E: Numeric, IO = Read> {
    state: Arc<dyn VirtualTensorOperations<E>>,
    _p: PhantomData<IO>,
}

/// Expand type for [VirtualTensor].
#[derive(Clone)]
pub struct VirtualTensorExpand<E: Numeric, IO> {
    state: Arc<dyn VirtualTensorOperationsExpand<E>>,
    _p: PhantomData<IO>,
}

#[cube]
impl<E: Numeric, IO: Clone> VirtualTensor<E, IO> {
    /// Read the tensor at the given index.
    pub fn read(&self, index: u32) -> Line<E> {
        self.state.read(index)
    }

    /// Get the shape of the tensor at the given axis.
    pub fn shape(&self, axis: u32) -> u32 {
        self.state.shape(axis)
    }

    /// Get the stride of the tensor at the given axis.
    pub fn stride(&self, axis: u32) -> u32 {
        self.state.stride(axis)
    }

    /// Get the rank of the tensor.
    pub fn rank(&self) -> u32 {
        self.state.rank()
    }
}

#[cube]
impl<E: Numeric> VirtualTensor<E, ReadWrite> {
    /// Write the tensor at the given index.
    pub fn write(&mut self, index: u32, value: Line<E>) {
        self.state.write(index, value)
    }
}

impl<E: Numeric> VirtualTensor<E, Read> {
    /// Create a new [read only](Read) [virtual tensor](VirtualTensor).
    pub fn new<V: VirtualTensorOperations<E> + 'static>(_v: &V) -> Self {
        unexpanded!()
    }

    /// Expand function of [Self::new].
    pub fn __expand_new<V>(
        _context: &mut CubeContext,
        v: V::ExpandType,
    ) -> VirtualTensorExpand<E, Read>
    where
        V::ExpandType: VirtualTensorOperationsExpand<E>,
        V: VirtualTensorOperations<E> + CubeType + 'static,
    {
        VirtualTensorExpand {
            state: Arc::new(v),
            _p: PhantomData,
        }
    }
}

impl<E: Numeric> VirtualTensor<E, ReadWrite> {
    /// Create a new [read write](ReadWrite) [virtual tensor](VirtualTensor).
    pub fn new<V: VirtualTensorOperations<E> + 'static>(_v: &mut V) -> Self {
        unexpanded!()
    }

    /// Expand function of [Self::new].
    pub fn __expand_new<V>(
        _context: &mut CubeContext,
        v: V::ExpandType,
    ) -> VirtualTensorExpand<E, ReadWrite>
    where
        V::ExpandType: VirtualTensorOperationsExpand<E>,
        V: VirtualTensorOperations<E> + CubeType + 'static,
    {
        VirtualTensorExpand {
            state: Arc::new(v),
            _p: PhantomData,
        }
    }
}

/// Trait to be implemented by a type that can become a [virtual tensor](VirtualTensor).
///
/// The [expand trait](VirtualTensorOperationsExpand) also need to be implemented for the type's
/// expand type.
///
/// # Warning
///
/// This trait is kind of unsafe, [VirtualTensorOperations::write] doesn't follow the mutability
/// rules, but it won't lead to any undefined behavior.
pub trait VirtualTensorOperations<E: Numeric> {
    /// Read the tensor at the given index.
    fn read(&self, _index: u32) -> Line<E> {
        unexpanded!()
    }
    /// Write the tensor at the given index.
    fn write(&self, _index: u32, _value: Line<E>) {
        unexpanded!()
    }
    /// Get the shape of the tensor at the given axis.
    fn shape(&self, _axis: u32) -> u32 {
        unexpanded!()
    }
    /// Get the stride of the tensor at the given axis.
    fn stride(&self, _axis: u32) -> u32 {
        unexpanded!()
    }
    /// Get the rank of the tensor.
    fn rank(&self) -> u32 {
        unexpanded!()
    }
}

/// Expand trait for [VirtualTensorOperations].
///
/// For now this needs to be manually implemented for any type that needs to become a virtual
/// tensor.
pub trait VirtualTensorOperationsExpand<E: Numeric> {
    fn __expand_read_method(
        &self,
        context: &mut CubeContext,
        index: ExpandElementTyped<u32>,
    ) -> ExpandElementTyped<Line<E>>;
    fn __expand_write_method(
        &self,
        context: &mut CubeContext,
        index: ExpandElementTyped<u32>,
        value: ExpandElementTyped<Line<E>>,
    );
    fn __expand_shape_method(
        &self,
        context: &mut CubeContext,
        axis: ExpandElementTyped<u32>,
    ) -> ExpandElementTyped<u32>;
    fn __expand_stride_method(
        &self,
        context: &mut CubeContext,
        axis: ExpandElementTyped<u32>,
    ) -> ExpandElementTyped<u32>;
    fn __expand_rank_method(&self, context: &mut CubeContext) -> ExpandElementTyped<u32>;
}

/// Making [virtual tensors](VirtualTensor) a proper [cube type](CubeType).
mod __cube_type {
    use super::*;

    impl<E: Numeric, IO: Clone> CubeType for VirtualTensor<E, IO> {
        type ExpandType = VirtualTensorExpand<E, IO>;
    }

    impl<E: Numeric, IO> Init for VirtualTensorExpand<E, IO> {
        fn init(self, _context: &mut CubeContext) -> Self {
            self
        }
    }

    impl<E: Numeric, IO: Clone> IntoRuntime for VirtualTensor<E, IO> {
        fn __expand_runtime_method(self, _context: &mut CubeContext) -> Self::ExpandType {
            panic!("Virtual tensors don't exist at comptime.")
        }
    }
}

/// Enable tensors to be virtual.
mod __tensor {
    use super::*;

    impl<E: Numeric> VirtualTensorOperations<E> for Tensor<Line<E>> {}
    impl<E: Numeric> VirtualTensorOperationsExpand<E> for ExpandElementTyped<Tensor<Line<E>>> {
        fn __expand_read_method(
            &self,
            context: &mut CubeContext,
            index: ExpandElementTyped<u32>,
        ) -> ExpandElementTyped<Line<E>> {
            self.clone().__expand_index_unchecked_method(context, index)
        }

        fn __expand_write_method(
            &self,
            context: &mut CubeContext,
            index: ExpandElementTyped<u32>,
            value: ExpandElementTyped<Line<E>>,
        ) {
            self.clone()
                .__expand_index_assign_unchecked_method(context, index, value)
        }

        fn __expand_shape_method(
            &self,
            context: &mut CubeContext,
            axis: ExpandElementTyped<u32>,
        ) -> ExpandElementTyped<u32> {
            self.clone().__expand_shape_method(context, axis)
        }

        fn __expand_stride_method(
            &self,
            context: &mut CubeContext,
            axis: ExpandElementTyped<u32>,
        ) -> ExpandElementTyped<u32> {
            self.clone().__expand_stride_method(context, axis)
        }

        fn __expand_rank_method(&self, context: &mut CubeContext) -> ExpandElementTyped<u32> {
            self.clone().__expand_rank_method(context)
        }
    }
}