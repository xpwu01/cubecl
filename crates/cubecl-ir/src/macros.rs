use crate::ConstantScalarValue;
use cubecl_common::{flex32, tf32};
use half::{bf16, f16};

use super::{FloatKind, IntKind, UIntKind, Variable};

#[macro_export]
/// Cube Pseudo Assembly.
macro_rules! cpa {
    // out = lhs + rhs
    ($scope:expr, $out:ident = $lhs:ident + $rhs:expr) => {
        cpa!($scope, $out = add($lhs, $rhs))
    };
    // out += input
    ($scope:expr, $out:ident += $input:ident) => {
        cpa!($scope, $out = add($out, $input))
    };
    // out = add(lhs, rhs)
    ($scope:expr, $out:ident = add($lhs:expr, $rhs:expr)) => {
        $scope.register($crate::Instruction::new($crate::Arithmetic::Add(
            cpa!(binary $lhs, $rhs)
        ), $out));
    };
    // out = lhs - rhs
    ($scope:expr, $out:ident = $lhs:ident - $rhs:expr) => {
        cpa!($scope, $out = sub($lhs, $rhs))
    };
    // out = sub(lhs, rhs)
    ($scope:expr, $out:ident = sub($lhs:expr, $rhs:expr)) => {
        $scope.register($crate::Instruction::new($crate::Arithmetic::Sub(
            cpa!(binary $lhs, $rhs)
        ), $out));
    };
    // out = lhs * rhs
    ($scope:expr, $out:ident = $lhs:ident * $rhs:expr) => {
        cpa!($scope, $out = mul($lhs, $rhs))
    };
    // out *= input
    ($scope:expr, $out:ident *= $input:ident) => {
        cpa!($scope, $out = mul($out, $input))
    };
    // out = mul(lhs, rhs)
    ($scope:expr, $out:ident = mul($lhs:expr, $rhs:expr)) => {
        $scope.register($crate::Instruction::new($crate::Arithmetic::Mul(
            cpa!(binary $lhs, $rhs)
        ), $out));
    };
    // out = lhs / rhs
    ($scope:expr, $out:ident = $lhs:ident / $rhs:expr) => {
        cpa!($scope, $out = div($lhs, $rhs))
    };
    // out = div(lhs, rhs)
    ($scope:expr, $out:ident = div($lhs:expr, $rhs:expr)) => {
        $scope.register($crate::Instruction::new($crate::Arithmetic::Div(
            cpa!(binary $lhs, $rhs)
        ), $out));
    };
    // out = lhs % rhs
    ($scope:expr, $out:ident = $lhs:ident % $rhs:expr) => {
        cpa!($scope, $out = modulo($lhs, $rhs))
    };
    // out = modulo(lhs, rhs)
    ($scope:expr, $out:ident = modulo($lhs:expr, $rhs:expr)) => {
        $scope.register($crate::Instruction::new($crate::Arithmetic::Modulo(
            cpa!(binary $lhs, $rhs)
        ), $out));
    };
    // out = powf(lhs, rhs)
    ($scope:expr, $out:ident = powf($lhs:expr, $rhs:expr)) => {
        $scope.register($crate::Instruction::new($crate::Arithmetic::Powf(
            cpa!(binary $lhs, $rhs)
        ), $out));
    };
    // out = lhs && rhs
    ($scope:expr, $out:ident = $lhs:ident && $rhs:expr) => {
        cpa!($scope, $out = and($lhs, $rhs))
    };
    // out = and(lhs, rhs)
    ($scope:expr, $out:ident = and($lhs:expr, $rhs:expr)) => {
        $scope.register($crate::Instruction::new($crate::Arithmetic::And(
            cpa!(binary $lhs, $rhs)
        ), $out));
    };
    // out = lhs || rhs
    ($scope:expr, $out:ident = $lhs:ident || $rhs:expr) => {
        cpa!($scope, $out = or($lhs, $rhs))
    };
    // out = or(lhs, rhs)
    ($scope:expr, $out:ident = or($lhs:expr, $rhs:expr)) => {
        $scope.register($crate::Instruction::new($crate::Arithmetic::Or(
            cpa!(binary $lhs, $rhs)
        ), $out));
    };
    // out = !input
    ($scope:expr, $out:ident = !$input:expr) => {
        cpa!($scope, $out = not($input))
    };
    // out = not(input)
    ($scope:expr, $out:ident = not($input:expr)) => {
        $scope.register($crate::Instruction::new($crate::Arithmetic::Not(
            cpa!(unary $input)
        ), $out));
    };
    // out = lhs & rhs
    ($scope:expr, $out: ident = $lhs:ident & $rhs:ident) => {
        cpa!($scope, $out = bitwise_and($lhs, $rhs))
    };
    // out = bitwise_and(lhs, rhs)
    ($scope:expr, $out:ident = bitwise_and($lhs:expr, $rhs:expr)) => {
        $scope.register($crate::Instruction::new($crate::Bitwise::BitwiseAnd(
            cpa!(binary $lhs, $rhs)
        ), $out));
    };
    // out = lhs ^ rhs
    ($scope:expr, $out: ident = $lhs:ident ^ $rhs:ident) => {
        cpa!($scope, $out = bitwise_xor($lhs, $rhs))
    };
    // out = bitwise_xor(lhs, rhs)
    ($scope:expr, $out:ident = bitwise_xor($lhs:expr, $rhs:expr)) => {
        $scope.register($crate::Instruction::new($crate::Bitwise::BitwiseXor(
            cpa!(binary $lhs, $rhs)
        ), $out));
    };
    // out = select(cond, then, or_else)
    ($scope:expr, $out:ident = select($cond:expr, $then:expr, $or_else:expr)) => {
        $scope.register($crate::Instruction::new($crate::Arithmetic::Select($crate::Select{
            cond: $cond,
            then: $then,
            or_else: $or_else,
        }), $out));
    };
    // out = lhs << rhs
    ($scope:expr, $out: ident = $lhs:ident << $rhs:ident) => {
        cpa!($scope, $out = shift_left($lhs, $rhs))
    };
    // out = shift_left(lhs, rhs)
    ($scope:expr, $out:ident = shift_left($lhs:expr, $rhs:expr)) => {
        $scope.register($crate::Instruction::new($crate::Bitwise::ShiftLeft(
            cpa!(binary $lhs, $rhs)
        ), $out));
    };
    // out = lhs >> rhs
    ($scope:expr, $out: ident = $lhs:ident >> $rhs:ident) => {
        cpa!($scope, $out = shift_right($lhs, $rhs))
    };
    // out = shift_right(lhs, rhs)
    ($scope:expr, $out:ident = shift_right($lhs:expr, $rhs:expr)) => {
        $scope.register($crate::Instruction::new($crate::Bitwise::ShiftRight(
            cpa!(binary $lhs, $rhs)
        ), $out));
    };
    // out = lhs == rhs
    ($scope:expr, $out:ident = $lhs:ident == $rhs:expr) => {
        cpa!($scope, $out = equal($lhs, $rhs))
    };
    // out = equal(lhs, rhs)
    ($scope:expr, $out:ident = equal($lhs:expr, $rhs:expr)) => {
        $scope.register($crate::Instruction::new($crate::Comparison::Equal(
            cpa!(binary $lhs, $rhs)
        ), $out));
    };
    // out = lhs != rhs
    ($scope:expr, $out:ident = $lhs:ident != $rhs:expr) => {
        cpa!($scope, $out = not_equal($lhs, $rhs))
    };
    // out = not_equal(lhs, rhs)
    ($scope:expr, $out:ident = not_equal($lhs:expr, $rhs:expr)) => {
        $scope.register($crate::Instruction::new($crate::Comparison::NotEqual(
            cpa!(binary $lhs, $rhs)
        ), $out));
    };
    // out = lhs > rhs
    ($scope:expr, $out:ident = $lhs:ident > $rhs:expr) => {
        cpa!($scope, $out = greater($lhs, $rhs))
    };
    // out = greater(lhs, rhs)
    ($scope:expr, $out:ident = greater($lhs:expr, $rhs:expr)) => {
        $scope.register($crate::Instruction::new($crate::Comparison::Greater(
            cpa!(binary $lhs, $rhs)
        ), $out));
    };
    // out = lhs >= rhs
    ($scope:expr, $out:ident = $lhs:ident >= $rhs:expr) => {
        cpa!($scope, $out = greater_equal($lhs, $rhs))
    };
    // out = greater_equal(lhs, rhs)
    ($scope:expr, $out:ident = greater_equal($lhs:expr, $rhs:expr)) => {
        $scope.register($crate::Instruction::new($crate::Comparison::GreaterEqual(
            cpa!(binary $lhs, $rhs)
        ), $out));
    };
    // out = lhs < rhs
    ($scope:expr, $out:ident = $lhs:ident < $rhs:expr) => {
        cpa!($scope, $out = lower($lhs, $rhs))
    };
    // out = lower(lhs, rhs)
    ($scope:expr, $out:ident = lower($lhs:expr, $rhs:expr)) => {
        $scope.register($crate::Instruction::new($crate::Comparison::Lower(
            cpa!(binary $lhs, $rhs)
        ), $out));
    };
    // out = lhs <= rhs
    ($scope:expr, $out:ident = $lhs:ident <= $rhs:expr) => {
        cpa!($scope, $out = lower_equal($lhs, $rhs))
    };
    // out = lower_equal(lhs, rhs)
    ($scope:expr, $out:ident = lower_equal($lhs:expr, $rhs:expr)) => {
        $scope.register($crate::Instruction::new($crate::Comparison::LowerEqual(
            cpa!(binary $lhs, $rhs)
        ), $out));
    };
    // out = max(lhs, rhs)
    ($scope:expr, $out:ident = max($lhs:expr, $rhs:expr)) => {
        $scope.register($crate::Instruction::new($crate::Arithmetic::Max(
            cpa!(binary $lhs, $rhs)
        ), $out));
    };
    // out = min(lhs, rhs)
    ($scope:expr, $out:ident = min($lhs:expr, $rhs:expr)) => {
        $scope.register($crate::Instruction::new($crate::Arithmetic::Min(
            cpa!(binary $lhs, $rhs)
        ), $out));
    };
    // out = lhs[rhs]
    ($scope:expr, $out:ident = $lhs:ident[$rhs:expr]) => {
        cpa!($scope, $out = index($lhs, $rhs))
    };
    // out = index(lhs, rhs)
    ($scope:expr, $out:ident = index($lhs:expr, $rhs:expr)) => {
        $scope.register($crate::Instruction::new($crate::Operator::Index(
            cpa!(binary $lhs, $rhs)
        ), $out));
    };
    // out = unchecked(lhs[rhs])
    ($scope:expr, $out:ident = unchecked($lhs:ident[$rhs:expr])) => {
        $scope.register($crate::Instruction::new($crate::Operator::UncheckedIndex(
            cpa!(binary $lhs, $rhs)
        ), $out));
    };
    // out[lhs] = rhs
    ($scope:expr, $out:ident[$lhs:ident] = $rhs:expr) => {
        $scope.register($crate::Instruction::new($crate::Operator::IndexAssign(
            cpa!(binary $lhs, $rhs)
        ), $out));
    };
    // unchecked(out[lhs]) = rhs
    ($scope:expr, unchecked($out:ident[$lhs:ident]) = $rhs:expr) => {
        $scope.register($crate::Instruction::new($crate::Operator::UncheckedIndexAssign(
            cpa!(binary $lhs, $rhs)
        ), $out));
    };
    // out = |input|
    ($scope:expr, $out:ident = |$input:ident|) => {
        cpa!($scope, $out = abs($input))
    };
    // out = abs(input)
    ($scope:expr, $out:ident = abs($input:expr)) => {
        $scope.register($crate::Instruction::new($crate::Arithmetic::Abs(
            cpa!(unary $input)
        ), $out));
    };
    // out = exp(input)
    ($scope:expr, $out:ident = exp($input:expr)) => {
        $scope.register($crate::Instruction::new($crate::Arithmetic::Exp(
            cpa!(unary $input)
        ), $out));
    };
    // out = log(input)
    ($scope:expr, $out:ident = log($input:expr)) => {
        $scope.register($crate::Instruction::new($crate::Arithmetic::Log(
            cpa!(unary $input)
        ), $out));
    };
    // out = log1p(input)
    ($scope:expr, $out:ident = log1p($input:expr)) => {
        $scope.register($crate::Instruction::new($crate::Arithmetic::Log1p(
            cpa!(unary $input)
        ), $out));
    };
    // out = cos(input)
    ($scope:expr, $out:ident = cos($input:expr)) => {
        $scope.register($crate::Instruction::new($crate::Arithmetic::Cos(
            cpa!(unary $input)
        ), $out));
    };
    ($scope:expr, $out:ident = normalize($input:expr)) => {
        $scope.register($crate::Instruction::new($crate::Arithmetic::Normalize(
            cpa!(unary $input)
        ), $out));
    };
    // out = sin(input)
    ($scope:expr, $out:ident = sin($input:expr)) => {
        $scope.register($crate::Instruction::new($crate::Arithmetic::Sin(
            cpa!(unary $input)
        ), $out));
    };
    // out = tanh(input)
    ($scope:expr, $out:ident = tanh($input:expr)) => {
        $scope.register($crate::Instruction::new($crate::Arithmetic::Tanh(
            cpa!(unary $input)
        ), $out));
    };
    // out = sqrt(input)
    ($scope:expr, $out:ident = sqrt($input:expr)) => {
        $scope.register($crate::Instruction::new($crate::Arithmetic::Sqrt(
            cpa!(unary $input)
        ), $out));
    };
    // out = floor(input)
    ($scope:expr, $out:ident = floor($input:expr)) => {
        $scope.register($crate::Instruction::new($crate::Arithmetic::Floor(
            cpa!(unary $input)
        ), $out));
    };
    // out = ceil(input)
    ($scope:expr, $out:ident = ceil($input:expr)) => {
        $scope.register($crate::Instruction::new($crate::Arithmetic::Ceil(
            cpa!(unary $input)
        ), $out));
    };
    // out = erf(input)
    ($scope:expr, $out:ident = erf($input:expr)) => {
        $scope.register($crate::Instruction::new($crate::Arithmetic::Erf(
            cpa!(unary $input)
        ), $out));
    };
    // out = input
    ($scope:expr, $out:ident = $input:ident) => {
        $scope.register($crate::Instruction::new($crate::Operation::Copy(
            $input
        ), $out));
    };
    // out = vec4(a, b, c, d)
    ($scope:expr, $out:ident = vec4($a:ident,$b:ident,$c:ident,$d:ident)) => {
        let i = $scope.zero(Elem::UInt);
        cpa!($scope, $out[i] = $a);
        cpa!($scope, i = i + 1u32);
        cpa!($scope, $out[i] = $b);
        cpa!($scope, i = i + 1u32);
        cpa!($scope, $out[i] = $c);
        cpa!($scope, i = i + 1u32);
        cpa!($scope, $out[i] = $d);
    };
    // out = input
    ($scope:expr, $out:ident = $input:ident) => {
        cpa!($scope, $out = cast($input))
    };
    // out = cast(input)
    ($scope:expr, $out:ident = cast($input:expr)) => {
        $scope.register($crate::Instruction::new($crate::Arithmetic::Cast(
            cpa!(unary $input)
        ), $out));
    };
    // out = shape(tensor, dim)
    ($scope:expr, $out:ident = shape($input:expr, $dim:expr)) => {
        $scope.register($crate::Instruction::new($crate::Metadata::Shape {
            dim: $dim.into(),
            var: $input.into(),
        }, $out));
    };
    // out = stride(tensor, dim)
    ($scope:expr, $out:ident = stride($input:expr, $dim:expr)) => {
        $scope.register($crate::Instruction::new($crate::Metadata::Stride {
            dim: $dim.into(),
            var: $input.into(),
        }, $out));
    };
    // out = len(array)
    ($scope:expr, $out:ident = len($input:expr)) => {
        $scope.register($crate::Instruction::new($crate::Metadata::Length {
            var: $input.into(),
        }, $out));
    };
    // out = buffer_len(array)
    ($scope:expr, $out:ident = buffer_len($input:expr)) => {
        $scope.register($crate::Instruction::new($crate::Metadata::BufferLength {
            var: $input.into(),
        }, $out));
    };
    // range(start, end).for_each(|i, scope| { ... })
    ($scope:expr, range($start:expr, $end:expr).for_each($arg:expr)) => {
        $crate::RangeLoop::register($scope, $start.into(), $end.into(), None, false, $arg);
    };
    // range(start, end, unroll).for_each(|i, scope| { ... })
    ($scope:expr, range($start:expr, $end:expr, $unroll:expr).for_each($arg:expr)) => {
        if $unroll {
            $crate::UnrolledRangeLoop::register($scope, $start.into(), $end.into(), None, false, $arg);
        } else {
            $crate::RangeLoop::register($scope, $start.into(), $end.into(), None, false, $arg);
        }
    };
        // range_stepped(start, end, step).for_each(|i, scope| { ... })
        ($scope:expr, range($start:expr, $end:expr, $step:expr).for_each($arg:expr)) => {
            $crate::RangeLoop::register($scope, $start.into(), $end.into(), Some($step), $arg);
        };
        // range_stepped(start, end, step, unroll).for_each(|i, scope| { ... })
        ($scope:expr, range($start:expr, $end:expr, $step:expr, $unroll:expr).for_each($arg:expr)) => {
            if $unroll {
                $crate::UnrolledRangeLoop::register($scope, $start.into(), $end.into(), Some($step), $arg);
            } else {
                $crate::RangeLoop::register($scope, $start.into(), $end.into(), Some($step), $arg);
            }
        };
    // loop(|scope| { ... })
    ($scope:expr, loop($arg:expr)) => {
        $crate::Loop::register($scope, $arg);
    };
    // if (cond).then(|scope| { ... })
    ($scope:expr, if ($cond:expr).then($arg:expr)) => {
        $crate::If::register($scope, $cond.into(), $arg);
    };
    // if (cond).then(|scope| { ... }).else(|scope| { ... })
    ($scope:expr, if ($cond:expr).then($arg_if:expr).else($arg_else:expr)) => {
        $crate::IfElse::register($scope, $cond.into(), $arg_if, $arg_else);
    };
    (binary $lhs:expr, $rhs:expr) => {
        $crate::BinaryOperator {
            lhs: $lhs.into(),
            rhs: $rhs.into(),
        }
    };
    (unary $input:expr) => {
        $crate::UnaryOperator {
            input: $input.into(),
        }
    };
}

impl From<bool> for Variable {
    fn from(value: bool) -> Self {
        Variable::constant(ConstantScalarValue::Bool(value))
    }
}

impl From<i8> for Variable {
    fn from(value: i8) -> Self {
        Variable::constant(ConstantScalarValue::Int(value as i64, IntKind::I8))
    }
}

impl From<i16> for Variable {
    fn from(value: i16) -> Self {
        Variable::constant(ConstantScalarValue::Int(value as i64, IntKind::I16))
    }
}

impl From<i32> for Variable {
    fn from(value: i32) -> Self {
        Variable::constant(ConstantScalarValue::Int(value as i64, IntKind::I32))
    }
}

impl From<i64> for Variable {
    fn from(value: i64) -> Self {
        Variable::constant(ConstantScalarValue::Int(value, IntKind::I64))
    }
}

impl From<f16> for Variable {
    fn from(value: f16) -> Self {
        Variable::constant(ConstantScalarValue::Float(value.to_f64(), FloatKind::F16))
    }
}

impl From<bf16> for Variable {
    fn from(value: bf16) -> Self {
        Variable::constant(ConstantScalarValue::Float(value.to_f64(), FloatKind::BF16))
    }
}

impl From<flex32> for Variable {
    fn from(value: flex32) -> Self {
        Variable::constant(ConstantScalarValue::Float(
            value.to_f64(),
            FloatKind::Flex32,
        ))
    }
}

impl From<tf32> for Variable {
    fn from(value: tf32) -> Self {
        Variable::constant(ConstantScalarValue::Float(value.to_f64(), FloatKind::TF32))
    }
}

impl From<f32> for Variable {
    fn from(value: f32) -> Self {
        Variable::constant(ConstantScalarValue::Float(value as f64, FloatKind::F32))
    }
}

impl From<f64> for Variable {
    fn from(value: f64) -> Self {
        Variable::constant(ConstantScalarValue::Float(value, FloatKind::F64))
    }
}

impl From<u8> for Variable {
    fn from(value: u8) -> Self {
        Variable::constant(ConstantScalarValue::UInt(value as u64, UIntKind::U8))
    }
}

impl From<u16> for Variable {
    fn from(value: u16) -> Self {
        Variable::constant(ConstantScalarValue::UInt(value as u64, UIntKind::U16))
    }
}

impl From<u32> for Variable {
    fn from(value: u32) -> Self {
        Variable::constant(ConstantScalarValue::UInt(value as u64, UIntKind::U32))
    }
}

impl From<u64> for Variable {
    fn from(value: u64) -> Self {
        Variable::constant(ConstantScalarValue::UInt(value, UIntKind::U64))
    }
}

impl From<usize> for Variable {
    fn from(value: usize) -> Self {
        Variable::constant(ConstantScalarValue::UInt(value as u64, UIntKind::U32))
    }
}
