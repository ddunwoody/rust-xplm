// Licensed under either of:
/*
 * Apache License, Version 2.0:
 *
 * Copyright 2026 Sašo Kiselkov
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
 * Copyright (c) 2026 Sašo Kiselkov
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

use std::ffi::{CString, NulError};

use xplm_sys::{
    xplmFont_Basic, xplmFont_Proportional, XPLMDrawString, XPLMDrawTranslucentDarkBox,
    XPLMGetFontDimensions, XPLMMeasureString,
};

pub fn draw_translucent_dark_box(left: i32, top: i32, right: i32, bottom: i32) {
    unsafe { XPLMDrawTranslucentDarkBox(left, top, right, bottom) };
}

#[derive(Copy, Clone, Debug, Hash, PartialEq, Eq)]
#[repr(i32)]
pub enum FontID {
    Basic = xplmFont_Basic as _,
    Proportional = xplmFont_Proportional as _,
}

impl FontID {
    pub fn get_dimensions(self) -> FontDimensions {
        let (mut w, mut h, mut d) = (0, 0, 0);
        unsafe { XPLMGetFontDimensions(self as _, &mut w, &mut h, &mut d) };
        FontDimensions {
            char_width: w,
            char_height: h,
            digits_only: d != 0,
        }
    }
    pub fn measure_string<S: AsRef<str>>(self, text: S) -> f32 {
        let text = text.as_ref();
        let ptr = text.as_ptr();
        unsafe { XPLMMeasureString(self as _, ptr as *const _, text.len() as _) }
    }
}

#[derive(Copy, Clone, Debug, Default, Hash, PartialEq, Eq)]
pub struct FontDimensions {
    pub char_width: i32,
    pub char_height: i32,
    pub digits_only: bool,
}

pub fn draw_string<S: AsRef<str>>(
    color_rgb: &[f32; 3],
    x_off: i32,
    y_off: i32,
    text: S,
    mut word_wrap_width: Option<i32>,
    font: FontID,
) -> Result<(), NulError> {
    let mut color_rgb = *color_rgb;
    let text = CString::new(text.as_ref().as_bytes())?;
    let word_wrap_width: *mut i32 = if let Some(w) = word_wrap_width.as_mut() {
        w
    } else {
        std::ptr::null_mut()
    };
    unsafe {
        XPLMDrawString(
            color_rgb.as_mut_ptr(),
            x_off,
            y_off,
            text.as_ptr(),
            word_wrap_width,
            font as _,
        );
    }
    Ok(())
}
