use std::{
    cmp::Ordering,
    fmt::{self, Display},
};

#[derive(Debug, Clone, PartialEq)]
pub enum Value {
    None,
    Bool(bool),
    Integer(i64),
    Float(f64),
    Text(String),
}

impl PartialOrd<Value> for Value {
    fn partial_cmp(&self, other: &Value) -> Option<Ordering> {
        match (self, other) {
            (Value::None, Value::None) => Some(Ordering::Equal),
            (Value::None, _) => Some(Ordering::Less),
            (_, Value::None) => Some(Ordering::Greater),
            (Value::Bool(lhs), Value::Bool(rhs)) => lhs.partial_cmp(rhs),
            (Value::Integer(lhs), Value::Integer(rhs)) => lhs.partial_cmp(rhs),
            (Value::Float(lhs), Value::Float(rhs)) => lhs.partial_cmp(rhs),
            (Value::Text(lhs), Value::Text(rhs)) => lhs.partial_cmp(rhs),
            _ => None,
        }
    }
}

impl Display for Value {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Value::None => write!(f, ""),
            Value::Bool(value) => write!(f, "{value}"),
            Value::Integer(value) => write!(f, "{value}"),
            Value::Float(value) => {
                if value.is_finite() {
                    let log1000 = value.abs().log(1000.0);

                    // log10 is finite when the value is not zero.
                    if log1000.is_finite() {
                        // Compute the 10's exponent in multiple of 3
                        let exp1000 = log1000.floor() as i32;

                        if exp1000 == 0 {
                            write!(f, "{value:7.3}")
                        } else {
                            let fract = value * 1000f64.powi(-exp1000);
                            let exp10 = exp1000 * 3;
                            write!(f, "{fract:7.3}e{exp10}")
                        }
                    } else {
                        write!(f, "{value:7.3}")
                    }
                } else {
                    write!(f, "{value}")
                }
            }
            Value::Text(value) => write!(f, "{value}"),
        }
    }
}

impl From<&str> for Value {
    fn from(value: &str) -> Self {
        Self::Text(value.to_string())
    }
}

impl From<String> for Value {
    fn from(value: String) -> Self {
        Self::Text(value)
    }
}

impl From<&String> for Value {
    fn from(value: &String) -> Self {
        Self::Text(value.clone())
    }
}

impl From<bool> for Value {
    fn from(value: bool) -> Self {
        Self::Bool(value)
    }
}

impl From<i8> for Value {
    fn from(value: i8) -> Self {
        Self::Integer(value as i64)
    }
}

impl From<i16> for Value {
    fn from(value: i16) -> Self {
        Self::Integer(value as i64)
    }
}

impl From<i32> for Value {
    fn from(value: i32) -> Self {
        Self::Integer(value as i64)
    }
}

impl From<i64> for Value {
    fn from(value: i64) -> Self {
        Self::Integer(value)
    }
}

impl TryFrom<isize> for Value {
    type Error = isize;

    fn try_from(value: isize) -> Result<Self, Self::Error> {
        let Ok(value) = value.try_into() else {
            return Err(value);
        };
        Ok(Self::Integer(value))
    }
}

impl From<u8> for Value {
    fn from(value: u8) -> Self {
        Self::Integer(value as i64)
    }
}

impl From<u16> for Value {
    fn from(value: u16) -> Self {
        Self::Integer(value as i64)
    }
}

impl From<u32> for Value {
    fn from(value: u32) -> Self {
        Self::Integer(value as i64)
    }
}

impl TryFrom<u64> for Value {
    type Error = u64;

    fn try_from(value: u64) -> Result<Self, Self::Error> {
        let Ok(value) = value.try_into() else {
            return Err(value);
        };
        Ok(Self::Integer(value))
    }
}

impl TryFrom<usize> for Value {
    type Error = usize;

    fn try_from(value: usize) -> Result<Self, Self::Error> {
        let Ok(value) = value.try_into() else {
            return Err(value);
        };
        Ok(Self::Integer(value))
    }
}

impl From<f32> for Value {
    fn from(value: f32) -> Self {
        Self::Float(value as f64)
    }
}

impl From<f64> for Value {
    fn from(value: f64) -> Self {
        Self::Float(value)
    }
}

impl From<Option<f32>> for Value {
    fn from(value: Option<f32>) -> Self {
        match value {
            Some(value) => Self::Float(value as f64),
            None => Self::None,
        }
    }
}

impl From<Option<f64>> for Value {
    fn from(value: Option<f64>) -> Self {
        match value {
            Some(value) => Self::Float(value),
            None => Self::None,
        }
    }
}
