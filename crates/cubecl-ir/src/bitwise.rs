use std::fmt::Display;

use type_hash::TypeHash;

use crate::{BinaryOperator, OperationReflect, UnaryOperator};

/// Bitwise operations
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
#[derive(Debug, Clone, TypeHash, PartialEq, Eq, Hash, OperationReflect)]
#[operation(opcode_name = BitwiseOpCode, pure)]
pub enum Bitwise {
    #[operation(commutative)]
    BitwiseAnd(BinaryOperator),
    #[operation(commutative)]
    BitwiseOr(BinaryOperator),
    #[operation(commutative)]
    BitwiseXor(BinaryOperator),
    ShiftLeft(BinaryOperator),
    ShiftRight(BinaryOperator),
    CountOnes(UnaryOperator),
    ReverseBits(UnaryOperator),
    BitwiseNot(UnaryOperator),
}

impl Display for Bitwise {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Bitwise::BitwiseAnd(op) => write!(f, "{} & {}", op.lhs, op.rhs),
            Bitwise::BitwiseOr(op) => write!(f, "{} | {}", op.lhs, op.rhs),
            Bitwise::BitwiseXor(op) => write!(f, "{} ^ {}", op.lhs, op.rhs),
            Bitwise::CountOnes(op) => write!(f, "{}.count_bits()", op.input),
            Bitwise::ReverseBits(op) => write!(f, "{}.reverse_bits()", op.input),
            Bitwise::ShiftLeft(op) => write!(f, "{} << {}", op.lhs, op.rhs),
            Bitwise::ShiftRight(op) => write!(f, "{} >> {}", op.lhs, op.rhs),
            Bitwise::BitwiseNot(op) => write!(f, "!{}", op.input),
        }
    }
}
