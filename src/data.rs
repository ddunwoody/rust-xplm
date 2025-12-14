use crate::ffi::StringBuffer;
use std::ffi::{CString, NulError};
use std::string::FromUtf8Error;
use xplm_sys::*;

/// Datarefs created by X-Plane or other plugins
pub mod borrowed;
/// Datarefs created by this plugin
pub mod owned;

/// Marks a dataref as readable
pub enum ReadOnly {}

/// Marks a dataref as writeable
pub enum ReadWrite {}

/// Marker for data access types
pub trait Access {
    /// Returns true if this access allows the dataref to be written
    fn writeable() -> bool;
}

impl Access for ReadOnly {
    fn writeable() -> bool {
        false
    }
}

impl Access for ReadWrite {
    fn writeable() -> bool {
        true
    }
}

/// Trait for data accessors that can be read
pub trait DataRead<T> {
    /// Reads a value
    fn get(&self) -> T;
}

/// Trait for writable data accessors
pub trait DataReadWrite<T>: DataRead<T> {
    /// Writes a value
    fn set(&mut self, value: T);
}

/// Trait for readable array data accessors
#[allow(clippy::len_without_is_empty)]
pub trait ArrayRead<T: ArrayType + ?Sized> {
    /// Reads values
    ///
    /// Values are stored in the provided slice. If the dataref is larger than the provided slice,
    /// values beyond the bounds of the slice are ignored.
    ///
    /// If the dataref is smaller than the provided slice, the extra values in the slice will not
    /// be modified.
    ///
    /// The maximum number of values in an array dataref is i32::MAX.
    ///
    /// This function returns the number of values that were read.
    fn get(&self, dest: &mut [T::Element]) -> usize;

    /// Returns the length of the data array
    fn len(&self) -> usize;

    /// Returns all values in this accessor as a Vec
    fn as_vec(&self) -> Vec<T::Element>
    where
        T::Element: Default + Clone,
    {
        let mut values = vec![T::Element::default(); self.len()];
        self.get(&mut values);
        values
    }
}

/// Trait for array accessors that can be read and written
pub trait ArrayReadWrite<T: ArrayType + ?Sized>: ArrayRead<T> {
    /// Writes values
    ///
    /// Values are taken from the provided slice. If the dataref is larger than the provided slice,
    /// values beyond the bounds of the slice are not changed.
    ///
    /// If the dataref is smaller than the provided slice, the values beyond the dataref bounds
    /// will be ignored.
    fn set(&mut self, values: &[T::Element]);
}

/// Trait for data accessors that can be read as strings
pub trait StringRead {
    /// Reads the value of this dataref and appends it to the provided string
    ///
    /// Returns an error if the dataref is not valid UTF-8.
    ///
    /// If the provided string is not empty, the value of the dataref will be appended to it.
    fn get_to_string(&self, out: &mut String) -> Result<(), FromUtf8Error>;

    /// Reads the value of this dataref as a string and returns it
    fn get_as_string(&self) -> Result<String, FromUtf8Error>;
}

/// Trait for data accessors that can be written as strings
pub trait StringReadWrite: StringRead {
    /// Sets the value of this dataref from a string
    ///
    /// Returns an error if the string contains a null byte
    fn set_as_string(&mut self, value: &str) -> Result<(), NulError>;
}

impl<T> StringRead for T
where
    T: ArrayRead<[u8]>,
{
    fn get_to_string(&self, out: &mut String) -> Result<(), FromUtf8Error> {
        let mut buffer = StringBuffer::new(self.len());
        self.get(buffer.as_bytes_mut());
        let value_string = buffer.into_string()?;
        out.push_str(&value_string);
        Ok(())
    }
    fn get_as_string(&self) -> Result<String, FromUtf8Error> {
        let mut buffer = StringBuffer::new(self.len());
        self.get(buffer.as_bytes_mut());
        buffer.into_string()
    }
}

impl<T> StringReadWrite for T
where
    T: ArrayReadWrite<[u8]>,
{
    fn set_as_string(&mut self, value: &str) -> Result<(), NulError> {
        let name_c = CString::new(value)?;
        self.set(name_c.as_bytes_with_nul());
        Ok(())
    }
}

/// Marker for types that can be used with datarefs
pub trait DataType {
    /// The type that should be used to store data of this type
    /// For basic types, this is usually Self. For [T] types, this is Vec<T>.
    #[doc(hidden)]
    type Storage: Sized;
    /// The type used for validation.
    #[doc(hidden)]
    type Validation: Sized;
    /// Returns the X-Plane data type corresponding with this type
    #[doc(hidden)]
    fn sim_type() -> XPLMDataTypeID;
    /// Creates an instance of a storage type from an instance of self
    #[doc(hidden)]
    fn to_storage(&self) -> Self::Storage;
}

/// Marker for types that are arrays
pub trait ArrayType: DataType {
    /// The type of the array element
    type Element;
}

macro_rules! impl_type {
    ($native_type:ty as $sim_type:ident) => {
        impl DataType for $native_type {
            type Storage = Self;
            type Validation = $native_type;
            fn sim_type() -> XPLMDataTypeID {
                $sim_type as XPLMDataTypeID
            }
            fn to_storage(&self) -> Self::Storage {
                self.clone()
            }
        }
    };
    ([$native_type:ty]: array as $sim_type:ident) => {
        impl DataType for [$native_type] {
            type Storage = Vec<$native_type>;
            type Validation = $native_type;
            fn sim_type() -> XPLMDataTypeID {
                $sim_type as XPLMDataTypeID
            }
            fn to_storage(&self) -> Self::Storage {
                self.to_vec()
            }
        }
        impl ArrayType for [$native_type] {
            type Element = $native_type;
        }
    };
}

impl_type!(bool as xplmType_Int);
impl_type!(u8 as xplmType_Int);
impl_type!(i8 as xplmType_Int);
impl_type!(u16 as xplmType_Int);
impl_type!(i16 as xplmType_Int);
impl_type!(u32 as xplmType_Int);
impl_type!(i32 as xplmType_Int);
impl_type!(f32 as xplmType_Float);
impl_type!(f64 as xplmType_Double);
impl_type!([i32]: array as xplmType_IntArray);
impl_type!([u32]: array as xplmType_IntArray);
impl_type!([f32]: array as xplmType_FloatArray);
impl_type!([u8]: array as xplmType_Data);
impl_type!([i8]: array as xplmType_Data);

/// Various validators which can be used in `ValidatedDataRef` structs to provide data
/// validation functions.
pub mod validator {
    use std::{marker::PhantomData, num::FpCategory};

    /// Any data validator passed to `ValidatedDataRef` must implement this trait.
    /// Note that when the dataref is referencing an array, the `validate` function
    /// will be invoked for the individual elements of the array, instead of the
    /// array as a whole.
    pub trait DataValidator<T: ?Sized> {
        /// The error returned from the validator in case data validation failed.
        type Error;
        /// Called by the validator to validate individual data items.
        fn validate(data: &T) -> Result<(), Self::Error>;
    }

    /// Meta-validator which allows you to combine multiple validators. This
    /// validator returns success if both subvalidations succeed. For example
    /// to validate a dataref fits within the overlap of two numerical ranges:
    /// ```
    /// use xplm::data::validator::{self, DataValidator};
    /// type CheckRangeA = validator::RangeInclusive<0, 10>;
    /// type CheckRangeB = validator::RangeFrom<5>;
    /// type CombinedCheck = validator::And<i32, CheckRangeA, CheckRangeB>;
    /// // let dr: ValidatedDataRef<i32, CombinedCheck> = ValidatedDataRef::find("test");
    /// assert!(CombinedCheck::validate(&4).is_err());  // Just outside of 5..
    /// assert!(CombinedCheck::validate(&5).is_ok());   // Just within both ranges
    /// assert!(CombinedCheck::validate(&10).is_ok());  // Just within both ranges
    /// assert!(CombinedCheck::validate(&11).is_err()); // Just outside of 0..=10
    /// // Floating point data is also supported, but due to const generic argument
    /// // limits, ranges can only be specified as integers.
    /// type CombinedCheckF32 = validator::And<f32, CheckRangeA, CheckRangeB>;
    /// assert!(CombinedCheckF32::validate(&4.999).is_err());  // Just outside of 5..
    /// assert!(CombinedCheckF32::validate(&5.0).is_ok());     // Just within both ranges
    /// assert!(CombinedCheckF32::validate(&10.0).is_ok());    // Just within both ranges
    /// assert!(CombinedCheckF32::validate(&10.0001).is_err());// Just outside of 0..=10
    /// ```
    #[derive(Copy, Clone, Debug)]
    pub struct And<T, A: DataValidator<T>, B: DataValidator<T>> {
        validator_a: PhantomData<A>,
        validator_b: PhantomData<B>,
        data: PhantomData<T>,
    }
    impl<T, A, B> DataValidator<T> for And<T, A, B>
    where
        A: DataValidator<T>,
        B: DataValidator<T, Error = A::Error>,
    {
        type Error = A::Error;
        fn validate(data: &T) -> Result<(), Self::Error> {
            A::validate(data).and(B::validate(data))
        }
    }

    /// Meta-validator which allows you to combine multiple validators. This
    /// validator returns success if either subvalidation succeeds. For example
    /// to validate a dataref fits within one of two separate ranges:
    /// ```
    /// use xplm::data::validator::{self, DataValidator};
    /// type CheckRangeA = validator::RangeInclusive<0, 5>;
    /// type CheckRangeB = validator::RangeFrom<10>;
    /// type CombinedCheck = validator::Or<i32, CheckRangeA, CheckRangeB>;
    /// // let dr: ValidatedDataRef<i32, CombinedCheck> = ValidatedDataRef::find("test");
    /// assert!(CombinedCheck::validate(&5).is_ok());   // Within first range (0..=5)
    /// assert!(CombinedCheck::validate(&10).is_ok());  // Within second range (10..)
    /// assert!(CombinedCheck::validate(&-1).is_err()); // Within neither range
    /// assert!(CombinedCheck::validate(&7).is_err());  // Within neither range
    /// ```
    #[derive(Copy, Clone, Debug)]
    pub struct Or<T, A: DataValidator<T>, B: DataValidator<T>> {
        validator_a: PhantomData<A>,
        validator_b: PhantomData<B>,
        data: PhantomData<T>,
    }
    impl<T, A, B> DataValidator<T> for Or<T, A, B>
    where
        A: DataValidator<T>,
        B: DataValidator<T, Error = A::Error>,
    {
        type Error = A::Error;
        fn validate(data: &T) -> Result<(), Self::Error> {
            A::validate(data).or(B::validate(data))
        }
    }

    /// Validation error for numbers. This error enum is returned from the various
    /// numeric validators in this module.
    #[derive(Clone, Debug, thiserror::Error)]
    pub enum NumberValidationError<T> {
        /// Encountered a number which cannot be classified as a normal floating
        /// point number (i.e. `classify()` returned something other than
        /// `FpCategory::Normal`).
        #[error("number {0} is not a normal floating point number")]
        NotNormal(T),
        /// The number is not positive. This has to be a separate enum, because
        /// Rust's std::ops::RangeBounds lacks a left-exclusive range variant.
        #[error("number {0} is not positive")]
        NotPositive(T),
        /// The number does not fall within the required range.
        #[error("number {0} is not in the required range {1}")]
        NotInRange(T, RangeAny<T>),
    }

    #[derive(Copy, Clone, Debug)]
    pub struct NormalFloat {}
    impl<T: num::Float> DataValidator<T> for NormalFloat {
        type Error = NumberValidationError<T>;
        fn validate(data: &T) -> Result<(), Self::Error> {
            match data.classify() {
                FpCategory::Normal => Ok(()),
                _ => Err(NumberValidationError::NotNormal(*data)),
            }
        }
    }

    #[derive(Copy, Clone, Debug)]
    pub struct Positive {}
    impl<T: num::Num + num::Zero + Copy + PartialOrd> DataValidator<T> for Positive {
        type Error = NumberValidationError<T>;
        fn validate(data: &T) -> Result<(), Self::Error> {
            (*data > T::zero())
                .then_some(())
                .ok_or(NumberValidationError::NotPositive(*data))
        }
    }

    macro_rules! try_conv_from_i64 {
        ($T: ty, $value: expr) => {
            <$T>::from_i64($value).unwrap_or_else(|| {
                unreachable!("Cannot represent {} as type {}", $value, stringify!($T),)
            })
        };
    }
    #[derive(Copy, Clone, Debug)]
    pub struct Range<const START: i64, const END: i64> {}
    impl<T, const START: i64, const END: i64> DataValidator<T> for Range<START, END>
    where
        T: num::Num + Copy + PartialOrd + num::FromPrimitive,
    {
        type Error = NumberValidationError<T>;
        fn validate(data: &T) -> Result<(), Self::Error> {
            let start = try_conv_from_i64!(T, START);
            let end = try_conv_from_i64!(T, END);
            let rng = start..end;
            rng.contains(data)
                .then_some(())
                .ok_or(NumberValidationError::NotInRange(*data, rng.into()))
        }
    }
    #[derive(Copy, Clone, Debug)]
    pub struct RangeInclusive<const START: i64, const END: i64> {}
    impl<T, const START: i64, const END: i64> DataValidator<T> for RangeInclusive<START, END>
    where
        T: num::Num + Copy + PartialOrd + num::FromPrimitive,
    {
        type Error = NumberValidationError<T>;
        fn validate(data: &T) -> Result<(), Self::Error> {
            let start = try_conv_from_i64!(T, START);
            let end = try_conv_from_i64!(T, END);
            let rng = start..=end;
            rng.contains(data)
                .then_some(())
                .ok_or(NumberValidationError::NotInRange(*data, rng.into()))
        }
    }
    #[derive(Copy, Clone, Debug)]
    pub struct RangeFrom<const START: i64> {}
    impl<T, const START: i64> DataValidator<T> for RangeFrom<START>
    where
        T: num::Num + Copy + PartialOrd + num::FromPrimitive,
    {
        type Error = NumberValidationError<T>;
        fn validate(data: &T) -> Result<(), Self::Error> {
            let start = try_conv_from_i64!(T, START);
            let rng = start..;
            rng.contains(data)
                .then_some(())
                .ok_or(NumberValidationError::NotInRange(*data, rng.into()))
        }
    }
    #[derive(Copy, Clone, Debug)]
    pub struct RangeTo<const START: i64> {}
    impl<T, const END: i64> DataValidator<T> for RangeTo<END>
    where
        T: num::Num + Copy + PartialOrd + num::FromPrimitive,
    {
        type Error = NumberValidationError<T>;
        fn validate(data: &T) -> Result<(), Self::Error> {
            let end = try_conv_from_i64!(T, END);
            let rng = ..end;
            rng.contains(data)
                .then_some(())
                .ok_or(NumberValidationError::NotInRange(*data, rng.into()))
        }
    }
    #[derive(Copy, Clone, Debug)]
    pub struct RangeToInclusive<const START: i64> {}
    impl<T, const END: i64> DataValidator<T> for RangeToInclusive<END>
    where
        T: num::Num + Copy + PartialOrd + num::FromPrimitive,
    {
        type Error = NumberValidationError<T>;
        fn validate(data: &T) -> Result<(), Self::Error> {
            let end = try_conv_from_i64!(T, END);
            let rng = ..=end;
            rng.contains(data)
                .then_some(())
                .ok_or(NumberValidationError::NotInRange(*data, rng.into()))
        }
    }

    #[derive(Clone, Debug)]
    pub enum RangeAny<T> {
        Range(std::ops::Range<T>),
        RangeFrom(std::ops::RangeFrom<T>),
        RangeFull(std::ops::RangeFull),
        RangeInclusive(std::ops::RangeInclusive<T>),
        RangeTo(std::ops::RangeTo<T>),
        RangeToInclusive(std::ops::RangeToInclusive<T>),
    }
    macro_rules! impl_from_for_range_any {
        ($srctype:ty, $variant:ident) => {
            impl<T> From<$srctype> for RangeAny<T> {
                fn from(value: $srctype) -> Self {
                    Self::$variant(value)
                }
            }
        };
    }
    impl_from_for_range_any!(::std::ops::Range<T>, Range);
    impl_from_for_range_any!(::std::ops::RangeFrom<T>, RangeFrom);
    impl_from_for_range_any!(::std::ops::RangeFull, RangeFull);
    impl_from_for_range_any!(::std::ops::RangeInclusive<T>, RangeInclusive);
    impl_from_for_range_any!(::std::ops::RangeTo<T>, RangeTo);
    impl_from_for_range_any!(::std::ops::RangeToInclusive<T>, RangeToInclusive);
    #[cfg(test)]
    mod test {
        #[test]
        fn test_validate_enum() {
            use crate::data::borrowed::TypedDataRef;
            #[derive(derive_more::TryFrom)]
            #[try_from(repr)]
            #[repr(i32)]
            enum ValidValues {
                A,
                B,
                C,
            }
            let _dr = TypedDataRef::<i32, ValidValues>::find("test");
        }
    }
}
