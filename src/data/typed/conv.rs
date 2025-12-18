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

#[macro_export]
macro_rules! uom_typed_dataref {
    (
        dataref: $dr_typename:ident,
        conv: $conv_typename:ident,
        uom: $(::)?$($uom_type:ident)::+,
        phys_unit: $unit_name:ident,
        range: ($minval:literal..$maxval:literal),
        error: $error_type:ident $(,)?
    ) => {
        #[derive(Copy, Clone, Debug)]
        pub struct $conv_typename {}

        impl<V> $crate::data::typed::InputUnitConversion<V, $($uom_type)::*<::uom::si::SI<V>, V>>
            for $conv_typename
        where
            V: ::num::Num + ::uom::Conversion<V, T = V> + From<i16> + PartialOrd,
            ::uom::si::SI<V>: ::uom::si::Units<V>,
            $unit_name: ::uom::Conversion<V, T = V>,
        {
            type Error = $error_type;
            fn try_conv_in(value: V) -> Result<$($uom_type)::*<::uom::si::SI<V>, V>, Self::Error> {
                (value > V::from($minval) && value < V::from($maxval))
                    .then(|| $($uom_type)::*::new::<$unit_name>(value))
                    .ok_or($error_type {})
            }
        }
        impl<V> $crate::data::typed::OutputUnitConversion<$($uom_type)::*<::uom::si::SI<V>, V>, V>
            for $conv_typename
        where
            V: ::num::Num + ::uom::Conversion<V, T = V>,
            ::uom::si::SI<V>: ::uom::si::Units<V>,
            $unit_name: ::uom::Conversion<V, T = V>,
        {
            fn conv_out(value: $($uom_type)::*<::uom::si::SI<V>, V>) -> V {
                value.get::<$unit_name>()
            }
        }
        pub type $dr_typename<V, A = ReadOnly> = $crate::data::typed::borrowed::TypedDataRef<
            V,
            $($uom_type)::*<::uom::si::SI<V>, V>,
            $conv_typename,
            A,
        >;
    };
}

#[cfg(test)]
mod tests {
    use crate::data::{
        typed::{TypedDataRead, TypedDataReadWrite},
        ReadOnly,
    };
    use uom::si::{
        f32::ThermodynamicTemperature,
        thermodynamic_temperature::{degree_celsius, kelvin},
    };

    #[derive(Copy, Clone, Debug, Eq, PartialEq)]
    pub struct InvalidTemperature {}

    uom_typed_dataref!(
        dataref: TemperatureCelsiusDataRef,
        conv: TemperatureCelsiusConv,
        uom: uom::si::thermodynamic_temperature::ThermodynamicTemperature,
        phys_unit: degree_celsius,
        range: (-273_i16..5000_i16),
        error: InvalidTemperature,
    );

    #[test]
    fn test_temperature_dataref() {
        let _dr_lock = crate::test_stubs::DATAREF_SYS_LOCK.lock();

        let mut dr = TemperatureCelsiusDataRef::<f32>::find("test/f32")
            .unwrap()
            .writeable()
            .unwrap();
        dr.set(ThermodynamicTemperature::new::<degree_celsius>(5.0));
        assert_eq!(
            dr.get().unwrap(),
            ThermodynamicTemperature::new::<kelvin>(278.15)
        );
        // Try to shove something bad into the dataref to check input range checking
        dr.set(ThermodynamicTemperature::new::<kelvin>(0.0));
        assert_eq!(dr.get().unwrap_err(), InvalidTemperature {});
    }
}
