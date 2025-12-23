// Licensed under either of:
/*
 * Apache License, Version 2.0:
 *
 * Copyright 2025 Sašo Kiselkov
 *
 * Licensed under the Apache License, Version 2.0 (the "License");
 * you may not use this file except in compliance with the License.
 * You may obtain a copy of the License at
 *
 *      http://www.apache.org/licenses/LICENSE-2.0
 *
 * Unless required by applicable law or agreed to in writing, software
 * distributed under the License is distributed on an "AS IS" BASIS,
 * WITHOUT WARRANTIES OR CONDITIONS OF ANY KIND, either express or implied.
 * See the License for the specific language governing permissions and
 * limitations under the License.
 */
/*
 * MIT license:
 *
 * Copyright (c) 2025 Sašo Kiselkov
 *
 * Permission is hereby granted, free of charge, to any pony obtaining a
 * copy of this software and associated documentation files (the "Software"),
 * to deal in the Software without restriction, including without limitation
 * the rights to use, copy, modify, merge, publish, distribute, sublicense,
 * and/or sell copies of the Software, and to permit ponies to whom the
 * Software is furnished to do so, subject to the following conditions:
 *
 * The above copyright notice and this permission notice shall be included in
 * all copies or substantial portions of the Software.
 *
 * THE SOFTWARE IS PROVIDED "AS IS", WITHOUT WARRANTY OF ANY KIND, EXPRESS OR
 * IMPLIED, INCLUDING BUT NOT LIMITED TO THE WARRANTIES OF MERCHANTABILITY,
 * FITNESS FOR A PARTICULAR PURPOSE AND NONINFRINGEMENT. IN NO EVENT SHALL
 * THE AUTHORS OR COPYRIGHT HOLDERS BE LIABLE FOR ANY CLAIM, DAMAGES OR OTHER
 * LIABILITY, WHETHER IN AN ACTION OF CONTRACT, TORT OR OTHERWISE, ARISING
 * FROM, OUT OF OR IN CONNECTION WITH THE SOFTWARE OR THE USE OR OTHER
 * DEALINGS IN THE SOFTWARE.
 */

use std::marker::PhantomData;

use super::{ArrayReadWrite, ArrayType, DataReadWrite, DataType};

/// Datarefs created by X-Plane or other plugins.
pub mod borrowed;

/// Datarefs created by this plugin.
pub mod owned;

/// Must be implemented by scalar datarefs which provide validation on read.
pub trait ValidatedDataRead<T, V>
where
    T: super::DataType,
    V: Validator<T::Validation>,
{
    fn get(&self) -> Result<T, V::Error>;
}

/// Must be implemented by scalar datarefs which provide validation on write.
/// Read-only datarefs need not implement this trait.
pub trait ValidatedDataReadWrite<T, V>
where
    T: DataType,
    V: Validator<T::Validation>,
{
    fn set(&mut self, value: T) -> Result<(), V::Error>;
}

/// Must be implemented by array datarefs which provide validation on read.
#[allow(clippy::len_without_is_empty)]
pub trait ValidatedArrayRead<T, V>
where
    T: ArrayType + ?Sized,
    V: Validator<T::Validation>,
{
    fn get(&self) -> Result<Vec<T::Element>, V::Error> {
        self.get_subdata(..)
    }
    fn get_subdata(
        &self,
        range: impl std::ops::RangeBounds<usize>,
    ) -> Result<Vec<T::Element>, V::Error>;
    fn len(&self) -> usize;
}

/// Must be implemented by array datarefs which provide validation on write.
/// Read-only datarefs need not implement this trait.
pub trait ValidatedArrayReadWrite<T, V>
where
    T: ArrayType + ?Sized,
    V: Validator<T::Validation>,
{
    fn set(&mut self, values: &[T::Element]) -> Result<(), V::Error> {
        self.set_subdata(values, 0)
    }
    fn set_subdata(&mut self, values: &[T::Element], offset: usize) -> Result<(), V::Error>;
}

/// A dataref that first validates all input and output data before passing it on.
/// This can be used to avoid attempting to write junk into the dataref system, or
/// consuming junk written by somebody else.
///
/// This works the same as a normal [`crate::data::borrowed::DataRef`] struct, except the
/// second generic argument must be a struct which implements the [`Validator`] trait. See
/// [`crate::data::validated::validator`] for a list of ready-to-use data validators.
/// NOTE: you can combine multiple validators into logical structures using the
/// [`crate::data::validated::validator::And`] and [`crate::data::validated::validator::Or`]
/// meta-validators.
#[derive(Copy, Clone, Debug)]
pub struct ValidatedData<T, V, Dref>
where
    T: DataType + ?Sized,
    V: Validator<T::Validation>,
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
            V: Validator<$native_type>,
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
            V: Validator<$native_type>,
            Dref: ArrayReadWrite<[$native_type]>,
        {
            fn set_subdata(
                &mut self,
                values: &[$native_type],
                offset: usize,
            ) -> Result<(), V::Error> {
                if let Some(e) = values.iter().find_map(|value| V::validate(value).err()) {
                    return Err(e);
                }
                self.dr.set_subdata(values, offset);
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

/// Any data validator passed to [`borrowed::ValidatedDataRef`] must implement this trait.
/// Note that when the dataref is referencing an array, the `validate()` function
/// will be invoked for the individual elements of the array, instead of the
/// array as a whole.
pub trait Validator<T: ?Sized> {
    /// The error returned from the validator in case data validation failed.
    type Error: std::fmt::Debug;
    /// Called by the validator to validate individual data items.
    fn validate(data: &T) -> Result<(), Self::Error>;
}

/// Additional generic Range types implementing the [`std::ops::RangeBounds`] trait.
pub mod range {
    use std::fmt;

    /// A range which is exclusive both for the start and end bound. This has no
    /// direct analog in Rust's standard range syntax.
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
    impl<T: fmt::Debug> fmt::Debug for RangeExclusive<T> {
        fn fmt(&self, fmt: &mut fmt::Formatter<'_>) -> fmt::Result {
            write!(fmt, ">")?;
            self.start.fmt(fmt)?;
            write!(fmt, "..")?;
            self.end.fmt(fmt)?;
            Ok(())
        }
    }

    /// A range with an exclusive start bound and no end bound. This has no
    /// direct analog in Rust's standard range syntax.
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
    impl<T: fmt::Debug> fmt::Debug for RangeFromExclusive<T> {
        fn fmt(&self, fmt: &mut fmt::Formatter<'_>) -> fmt::Result {
            write!(fmt, ">")?;
            self.start.fmt(fmt)?;
            write!(fmt, "..")?;
            Ok(())
        }
    }
}

/// Pre-made validators which you can use with `ValidatedOwnedData` and `ValidateDataRef`
/// to perform automatic input/output validation.
pub mod validator {
    use std::marker::PhantomData;
    #[cfg(feature = "number_validation")]
    use std::num::FpCategory;

    use super::Validator;

    /// Meta-validator which allows you to combine multiple validators. This
    /// validator returns success if both subvalidations succeed. For example
    /// to validate a dataref fits within the overlap of two numerical ranges:
    /// ```no_run
    /// use xplm::data::validated::{validator, Validator};
    /// use xplm::data::validated::borrowed::ValidatedDataRef;
    ///
    /// // Check the numerical value of the dataref is within the range 0..=10 AND 5..
    /// type CheckRangeA = validator::RangeInclusive<0, 10>;
    /// type CheckRangeB = validator::RangeFrom<5>;
    /// type CombinedCheck = validator::And<i32, CheckRangeA, CheckRangeB>;
    ///
    /// let dr: ValidatedDataRef<i32, CombinedCheck> = ValidatedDataRef::find("test").unwrap();
    ///
    /// assert!(CombinedCheck::validate(&4).is_err());  // Just outside of 5..
    /// assert!(CombinedCheck::validate(&5).is_ok());   // Just within both ranges
    /// assert!(CombinedCheck::validate(&10).is_ok());  // Just within both ranges
    /// assert!(CombinedCheck::validate(&11).is_err()); // Just outside of 0..=10
    ///
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
    /// ```no_run
    /// use xplm::data::validated::{validator, Validator};
    /// use xplm::data::validated::borrowed::ValidatedDataRef;
    ///
    /// // Check the numerical value of the dataref is either 0..=5 OR 10..
    /// type CheckRangeA = validator::RangeInclusive<0, 5>;
    /// type CheckRangeB = validator::RangeFrom<10>;
    /// type CombinedCheck = validator::Or<i32, CheckRangeA, CheckRangeB>;
    ///
    /// let dr: ValidatedDataRef<i32, CombinedCheck> = ValidatedDataRef::find("test").unwrap();
    ///
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
    /// <div class="warning">
    ///
    /// Requires the `number_validation` feature.
    ///
    /// </div>
    #[cfg(feature = "number_validation")]
    #[derive(Clone, Debug, thiserror::Error)]
    pub enum NumberValidationError<T> {
        /// Encountered a number which cannot be classified as a normal floating
        /// point number (i.e. `classify()` returned something other than
        /// `FpCategory::Normal`).
        #[error("number {0} is not a normal floating point number")]
        NotNormal(T),
        /// The number does not fall within the required range.
        #[error("number {0} is not in the required range {1}")]
        NotInRange(T, RangeAny<T>),
    }

    /// Validator for floating point numbers, which returns success if the number
    /// is classified as a normal number (finite, non-NaN and non-denormal).
    /// <div class="warning">
    ///
    /// Requires the `number_validation` feature.
    ///
    /// </div>
    #[cfg(feature = "number_validation")]
    #[derive(Copy, Clone, Debug)]
    pub struct NormalFloat {}
    #[cfg(feature = "number_validation")]
    impl<T: num::Float + std::fmt::Debug> Validator<T> for NormalFloat {
        type Error = NumberValidationError<T>;
        fn validate(data: &T) -> Result<(), Self::Error> {
            match data.classify() {
                FpCategory::Normal => Ok(()),
                _ => Err(NumberValidationError::NotNormal(*data)),
            }
        }
    }
    #[cfg(feature = "number_validation")]
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
    /// <div class="warning">
    ///
    /// Requires the `number_validation` feature.
    ///
    /// </div>
    #[cfg(feature = "number_validation")]
    #[derive(Copy, Clone, Debug)]
    pub struct Range<const START: i64, const END: i64> {}
    #[cfg(feature = "number_validation")]
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
    /// <div class="warning">
    ///
    /// Requires the `number_validation` feature.
    ///
    /// </div>
    #[cfg(feature = "number_validation")]
    #[derive(Copy, Clone, Debug)]
    pub struct RangeExclusive<const START: i64, const END: i64> {}
    #[cfg(feature = "number_validation")]
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
    /// <div class="warning">
    ///
    /// Requires the `number_validation` feature.
    ///
    /// </div>
    #[cfg(feature = "number_validation")]
    #[derive(Copy, Clone, Debug)]
    pub struct RangeInclusive<const START: i64, const END: i64> {}
    #[cfg(feature = "number_validation")]
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
    /// <div class="warning">
    ///
    /// Requires the `number_validation` feature.
    ///
    /// </div>
    #[cfg(feature = "number_validation")]
    #[derive(Copy, Clone, Debug)]
    pub struct RangeFrom<const START: i64> {}
    #[cfg(feature = "number_validation")]
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
    /// <div class="warning">
    ///
    /// Requires the `number_validation` feature.
    ///
    /// </div>
    #[cfg(feature = "number_validation")]
    #[derive(Copy, Clone, Debug)]
    pub struct RangeFromExclusive<const START: i64> {}
    #[cfg(feature = "number_validation")]
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
    /// <div class="warning">
    ///
    /// Requires the `number_validation` feature.
    ///
    /// </div>
    #[cfg(feature = "number_validation")]
    #[derive(Copy, Clone, Debug)]
    pub struct RangeTo<const START: i64> {}
    #[cfg(feature = "number_validation")]
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
    /// <div class="warning">
    ///
    /// Requires the `number_validation` feature.
    ///
    /// </div>
    #[cfg(feature = "number_validation")]
    #[derive(Copy, Clone, Debug)]
    pub struct RangeToInclusive<const START: i64> {}
    #[cfg(feature = "number_validation")]
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

    /// Generic container to allow holding any range value which may be used to
    /// construct a numerical validator.
    /// <div class="warning">
    ///
    /// Requires the `number_validation` feature.
    ///
    /// </div>
    #[cfg(feature = "number_validation")]
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
            #[cfg(feature = "number_validation")]
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
}
