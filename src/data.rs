use crate::ffi::StringBuffer;
use std::ffi::{CString, NulError};
use std::marker::PhantomData;
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
    #[must_use]
    fn get(&self, dest: &mut [T::Element]) -> usize;

    #[must_use]
    fn get_subdata(&self, dest: &mut [T::Element], start_offset: usize) -> usize;

    /// Returns the length of the data array
    fn len(&self) -> usize;

    /// Returns all values in this accessor as a Vec
    fn as_vec(&self) -> Vec<T::Element>
    where
        T::Element: Default + Clone,
    {
        let mut values = vec![T::Element::default(); self.len()];
        let act_len = self.get(&mut values);
        values.truncate(act_len);
        values
    }

    /// Same as `as_vec()`, but allows you to specify a range of subdata to
    /// request from the accessor. The returned vector might be shorter than
    /// the request if the specified range exceeds the length of the dataref.
    fn as_vec_subdata(&self, range: impl std::ops::RangeBounds<usize>) -> Vec<T::Element>
    where
        T::Element: Default + Clone,
    {
        let start = match range.start_bound() {
            std::ops::Bound::Included(start) => *start,
            std::ops::Bound::Excluded(start) => *start + 1,
            std::ops::Bound::Unbounded => 0,
        };
        let end = match range.end_bound() {
            std::ops::Bound::Included(end) => *end + 1,
            std::ops::Bound::Excluded(end) => *end,
            std::ops::Bound::Unbounded => self.len(),
        };
        let req_len = end - start;
        let mut values = vec![T::Element::default(); req_len];
        let act_len = self.get_subdata(&mut values, start);
        values.truncate(act_len);
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

    fn set_subdata(&mut self, values: &[T::Element], offset: usize);
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
        let act_len = self.get(buffer.as_bytes_mut());
        buffer.truncate(act_len);
        let value_string = buffer.into_string()?;
        out.push_str(&value_string);
        Ok(())
    }
    fn get_as_string(&self) -> Result<String, FromUtf8Error> {
        let mut buffer = StringBuffer::new(self.len());
        let act_len = self.get(buffer.as_bytes_mut());
        buffer.truncate(act_len);
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

pub trait ValidatedDataRead<T, V>
where
    T: DataType,
    V: validator::Validator<T::Validation>,
{
    fn get(&self) -> Result<T, V::Error>;
}

pub trait ValidatedDataReadWrite<T, V>
where
    T: DataType,
    V: validator::Validator<T::Validation>,
{
    fn set(&mut self, value: T) -> Result<(), V::Error>;
}

#[allow(clippy::len_without_is_empty)]
pub trait ValidatedArrayRead<T, V>
where
    T: ArrayType + ?Sized,
    V: validator::Validator<T::Validation>,
{
    fn get(&self, dest: &mut [T::Element]) -> Result<usize, V::Error>;
    fn len(&self) -> usize;
}

pub trait ValidatedArrayReadWrite<T, V>
where
    T: ArrayType + ?Sized,
    V: validator::Validator<T::Validation>,
{
    fn set(&mut self, values: &[T::Element]) -> Result<(), V::Error>;
}

pub trait TypedDataRead<X, R>
where
    X: DataType,
    R: TryFrom<X::Storage>,
{
    fn get(&self) -> Result<R, R::Error>;
}

pub trait TypedArrayRead<X, R>
where
    X: ArrayType + ?Sized,
    R: TryFrom<X::Element>,
{
    fn get(&self) -> Result<Vec<R>, R::Error>;
    fn get_subdata(&self, range: impl std::ops::RangeBounds<usize>) -> Result<Vec<R>, R::Error>;
}

pub trait TypedDataReadWrite<X, R>
where
    X: DataType,
    R: Into<X::Storage>,
{
    fn set(&mut self, value: R);
}

pub trait TypedArrayReadWrite<X, R>
where
    X: ArrayType + ?Sized,
    R: Into<X::Element>,
{
    fn set(&mut self, values: impl Iterator<Item = R>);
    fn set_subdata(&mut self, values: impl Iterator<Item = R>, offset: usize);
}

/// A dataref that first validates all input and output data before passing it on.
/// This can be used to avoid attempting to write junk into the dataref system, or
/// consuming junk written by somebody else.
///
/// This works the same as a normal DataRef struct, except the second generic
/// argument must be a struct which implements the `Validator` trait. See
/// `crate::data::validator` for a list of ready-to-use data validators.
pub struct ValidatedData<T, V, Dref>
where
    T: DataType + ?Sized,
    V: validator::Validator<T::Validation>,
{
    dr: Dref,
    data: PhantomData<T>,
    validator: PhantomData<V>,
}

macro_rules! impl_validated_data {
    // Scalar version
    ($native_type:ty) => {
        impl<V, Dref> ValidatedDataReadWrite<$native_type, V>
            for ValidatedData<$native_type, V, Dref>
        where
            V: validator::Validator<$native_type>,
            Dref: DataReadWrite<$native_type>,
        {
            fn set(&mut self, value: $native_type) -> Result<(), V::Error> {
                V::validate(&value)?;
                self.dr.set(value);
                Ok(())
            }
        }
    };
    // Array version
    (array $native_type:ty) => {
        impl<V, Dref> ValidatedArrayReadWrite<[$native_type], V>
            for ValidatedData<[$native_type], V, Dref>
        where
            V: validator::Validator<$native_type>,
            Dref: ArrayReadWrite<[$native_type]>,
        {
            fn set(&mut self, values: &[$native_type]) -> Result<(), V::Error> {
                if let Some(e) = values.iter().find_map(|value| V::validate(value).err()) {
                    return Err(e);
                }
                self.dr.set(values);
                Ok(())
            }
        }
    };
}

impl_validated_data!(i8);
impl_validated_data!(u8);
impl_validated_data!(i16);
impl_validated_data!(u16);
impl_validated_data!(i32);
impl_validated_data!(u32);
impl_validated_data!(f32);
impl_validated_data!(f64);

impl_validated_data!(array i8);
impl_validated_data!(array u8);
impl_validated_data!(array i32);
impl_validated_data!(array u32);
impl_validated_data!(array f32);

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

pub mod range {
    use std::fmt;

    #[derive(Clone, Hash, PartialEq, Eq)]
    pub struct RangeExclusive<T> {
        pub start: T,
        pub end: T,
    }
    impl<T> std::ops::RangeBounds<T> for RangeExclusive<T> {
        fn start_bound(&self) -> std::ops::Bound<&T> {
            std::ops::Bound::Excluded(&self.start)
        }
        fn end_bound(&self) -> std::ops::Bound<&T> {
            std::ops::Bound::Excluded(&self.end)
        }
    }
    impl<Idx: fmt::Debug> fmt::Debug for RangeExclusive<Idx> {
        fn fmt(&self, fmt: &mut fmt::Formatter<'_>) -> fmt::Result {
            write!(fmt, ">")?;
            self.start.fmt(fmt)?;
            write!(fmt, "..")?;
            self.end.fmt(fmt)?;
            Ok(())
        }
    }

    #[derive(Clone, Hash, PartialEq, Eq)]
    pub struct RangeFromExclusive<T> {
        pub start: T,
    }
    impl<T> std::ops::RangeBounds<T> for RangeFromExclusive<T> {
        fn start_bound(&self) -> std::ops::Bound<&T> {
            std::ops::Bound::Excluded(&self.start)
        }
        fn end_bound(&self) -> std::ops::Bound<&T> {
            std::ops::Bound::Unbounded
        }
    }
    impl<Idx: fmt::Debug> fmt::Debug for RangeFromExclusive<Idx> {
        fn fmt(&self, fmt: &mut fmt::Formatter<'_>) -> fmt::Result {
            write!(fmt, ">")?;
            self.start.fmt(fmt)?;
            write!(fmt, "..")?;
            Ok(())
        }
    }
}

/// Various validators which can be used in `ValidatedDataRef` structs to provide data
/// validation functions.
pub mod validator {
    use std::{marker::PhantomData, num::FpCategory};

    /// Any data validator passed to `ValidatedDataRef` must implement this trait.
    /// Note that when the dataref is referencing an array, the `validate` function
    /// will be invoked for the individual elements of the array, instead of the
    /// array as a whole.
    pub trait Validator<T: ?Sized> {
        /// The error returned from the validator in case data validation failed.
        type Error: std::fmt::Debug;
        /// Called by the validator to validate individual data items.
        fn validate(data: &T) -> Result<(), Self::Error>;
    }

    /// Meta-validator which allows you to combine multiple validators. This
    /// validator returns success if both subvalidations succeed. For example
    /// to validate a dataref fits within the overlap of two numerical ranges:
    /// ```
    /// use xplm::data::validator::{self, Validator};
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
    pub struct And<T, A: Validator<T>, B: Validator<T>> {
        validator_a: PhantomData<A>,
        validator_b: PhantomData<B>,
        data: PhantomData<T>,
    }
    impl<T, A, B> Validator<T> for And<T, A, B>
    where
        A: Validator<T>,
        B: Validator<T, Error = A::Error>,
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
    /// use xplm::data::validator::{self, Validator};
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
    pub struct Or<T, A: Validator<T>, B: Validator<T>> {
        validator_a: PhantomData<A>,
        validator_b: PhantomData<B>,
        data: PhantomData<T>,
    }
    impl<T, A, B> Validator<T> for Or<T, A, B>
    where
        A: Validator<T>,
        B: Validator<T, Error = A::Error>,
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

    /// Validator for floating point numbers, which returns success if the number
    /// is classified as a normal number (finite, non-NaN and non-denormal).
    #[derive(Copy, Clone, Debug)]
    pub struct NormalFloat {}
    impl<T: num::Float + std::fmt::Debug> Validator<T> for NormalFloat {
        type Error = NumberValidationError<T>;
        fn validate(data: &T) -> Result<(), Self::Error> {
            match data.classify() {
                FpCategory::Normal => Ok(()),
                _ => Err(NumberValidationError::NotNormal(*data)),
            }
        }
    }

    macro_rules! try_conv_from_i64 {
        ($T: ty, $value: expr) => {
            <$T>::from_i64($value).unwrap_or_else(|| {
                unreachable!("Cannot represent {} as type {}", $value, stringify!($T),)
            })
        };
    }
    /// Provides a range validator equivalent to a half-open `START..END` range expression.
    /// The start and end bounds must be specified as generic constants when this type is
    /// used.
    #[derive(Copy, Clone, Debug)]
    pub struct Range<const START: i64, const END: i64> {}
    impl<T, const START: i64, const END: i64> Validator<T> for Range<START, END>
    where
        T: num::Num + Copy + PartialOrd + num::FromPrimitive + std::fmt::Debug,
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
    /// Provides a range validator, where both the start and end bound are open. This
    /// has no direct equivalent in Rust's range expressions. The start and end bounds
    /// must be specified as generic constants when this type is used.
    #[derive(Copy, Clone, Debug)]
    pub struct RangeExclusive<const START: i64, const END: i64> {}
    impl<T, const START: i64, const END: i64> Validator<T> for RangeExclusive<START, END>
    where
        T: num::Num + Copy + PartialOrd + num::FromPrimitive + std::fmt::Debug,
    {
        type Error = NumberValidationError<T>;
        fn validate(data: &T) -> Result<(), Self::Error> {
            use std::ops::RangeBounds;
            let start = try_conv_from_i64!(T, START);
            let end = try_conv_from_i64!(T, END);
            let rng = super::range::RangeExclusive { start, end };
            rng.contains(data)
                .then_some(())
                .ok_or(NumberValidationError::NotInRange(*data, rng.into()))
        }
    }
    /// Provides a range validator equivalent to an inclusive `START..=END` Rust range
    /// express. The start and end bounds must be specified as generic constants when
    /// this type is used.
    #[derive(Copy, Clone, Debug)]
    pub struct RangeInclusive<const START: i64, const END: i64> {}
    impl<T, const START: i64, const END: i64> Validator<T> for RangeInclusive<START, END>
    where
        T: num::Num + Copy + PartialOrd + num::FromPrimitive + std::fmt::Debug,
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
    /// Provides a range validator equivalent to a half-bounded `START..` Rust range
    /// expression. The start and end bounds must be specified as generic constants
    /// when this type is used.
    #[derive(Copy, Clone, Debug)]
    pub struct RangeFrom<const START: i64> {}
    impl<T, const START: i64> Validator<T> for RangeFrom<START>
    where
        T: num::Num + Copy + PartialOrd + num::FromPrimitive + std::fmt::Debug,
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
    /// Provides a range validator, where the start bound is exclusive and the end
    /// is unbounded. This has no direct equivalent in Rust's range expressions. The
    /// start bound must be specified as a generic constant when this type is used.
    #[derive(Copy, Clone, Debug)]
    pub struct RangeFromExclusive<const START: i64> {}
    impl<T, const START: i64> Validator<T> for RangeFromExclusive<START>
    where
        T: num::Num + Copy + PartialOrd + num::FromPrimitive + std::fmt::Debug,
    {
        type Error = NumberValidationError<T>;
        fn validate(data: &T) -> Result<(), Self::Error> {
            use std::ops::RangeBounds;
            let start = try_conv_from_i64!(T, START);
            let rng = super::range::RangeFromExclusive { start };
            rng.contains(data)
                .then_some(())
                .ok_or(NumberValidationError::NotInRange(*data, rng.into()))
        }
    }
    /// Provides a range validator equivalent to a half-bounded exclusive `..END`
    /// Rust range expression. The start and end bounds must be specified as
    /// generic constants when this type is used.
    #[derive(Copy, Clone, Debug)]
    pub struct RangeTo<const START: i64> {}
    impl<T, const END: i64> Validator<T> for RangeTo<END>
    where
        T: num::Num + Copy + PartialOrd + num::FromPrimitive + std::fmt::Debug,
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
    /// Provides a range validator equivalent to a half-bounded inclusive
    /// `..=END` Rust range expression. The start and end bounds must be
    /// specified as generic constants when this type is used.
    #[derive(Copy, Clone, Debug)]
    pub struct RangeToInclusive<const START: i64> {}
    impl<T, const END: i64> Validator<T> for RangeToInclusive<END>
    where
        T: num::Num + Copy + PartialOrd + num::FromPrimitive + std::fmt::Debug,
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
        Range(::std::ops::Range<T>),
        RangeExclusive(super::range::RangeExclusive<T>),
        RangeFrom(::std::ops::RangeFrom<T>),
        RangeFromExclusive(super::range::RangeFromExclusive<T>),
        RangeFull(::std::ops::RangeFull),
        RangeInclusive(::std::ops::RangeInclusive<T>),
        RangeTo(::std::ops::RangeTo<T>),
        RangeToInclusive(::std::ops::RangeToInclusive<T>),
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
    impl_from_for_range_any!(super::range::RangeExclusive<T>, RangeExclusive);
    impl_from_for_range_any!(::std::ops::RangeFrom<T>, RangeFrom);
    impl_from_for_range_any!(super::range::RangeFromExclusive<T>, RangeFromExclusive);
    impl_from_for_range_any!(::std::ops::RangeFull, RangeFull);
    impl_from_for_range_any!(::std::ops::RangeInclusive<T>, RangeInclusive);
    impl_from_for_range_any!(::std::ops::RangeTo<T>, RangeTo);
    impl_from_for_range_any!(::std::ops::RangeToInclusive<T>, RangeToInclusive);

    #[cfg(test)]
    mod test {
        use crate::data::{borrowed::DataRef, ValidatedDataReadWrite};

        #[test]
        fn test_validated_dataref() {
            use crate::data::{validator, ValidatedData};

            let mut dr: ValidatedData<i32, validator::Range<0, 5>, DataRef<_, _>> =
                ValidatedData::find("test/i32")
                    .unwrap()
                    .writeable()
                    .unwrap();
            assert!(dr.set(4).is_ok());
            assert!(dr.set(5).is_err());
        }
        #[test]
        fn test_typed_dataref() {
            use crate::data::borrowed::TypedDataRef;
            use crate::data::{
                TypedArrayRead, TypedArrayReadWrite, TypedDataRead, TypedDataReadWrite,
            };
            #[derive(derive_more::TryFrom, Copy, Clone, Debug, PartialEq, Eq)]
            #[try_from(repr)]
            #[repr(i32)]
            enum ValidValues {
                A,
                B,
                C,
            }
            impl From<ValidValues> for i32 {
                fn from(value: ValidValues) -> Self {
                    value as _
                }
            }
            impl From<ValidValues> for u8 {
                fn from(value: ValidValues) -> Self {
                    value as _
                }
            }
            let mut dr = TypedDataRef::<i32, ValidValues>::find("test/i32")
                .unwrap()
                .writeable()
                .unwrap();
            let en = ValidValues::C;
            dr.set(en);
            assert_ne!(dr.get().unwrap(), ValidValues::A);
            assert_ne!(dr.get().unwrap(), ValidValues::B);
            assert_eq!(dr.get().unwrap(), ValidValues::C);

            let mut array_dr = TypedDataRef::<[i32], ValidValues>::find("test/i32array")
                .unwrap()
                .writeable()
                .unwrap();
            let en = ValidValues::C;
            array_dr.set(std::iter::once(en));
            let en_out = array_dr.get_subdata(0..1).unwrap();
            assert_eq!(en_out, vec![ValidValues::C]);
        }
    }
}
