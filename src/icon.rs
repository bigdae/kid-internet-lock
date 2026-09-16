use std::ptr::null_mut;
use windows_sys::Win32::Graphics::Gdi::{
    CreateBitmap, CreateCompatibleDC, CreateDIBSection, DeleteDC, DeleteObject,
    BITMAPINFO, BITMAPINFOHEADER, BI_RGB, DIB_RGB_COLORS, HBITMAP, RGBQUAD,
};
use windows_sys::Win32::UI::WindowsAndMessaging::{
    CreateIconIndirect, DestroyIcon, HICON, ICONINFO,
};

pub struct DynamicIcons {
    pub green_icon: HICON,
    pub red_icon: HICON,
}

impl DynamicIcons {
    pub fn new() -> Result<Self, String> {
        let green = create_status_icon(true)?;
        let red = create_status_icon(false)?;
        Ok(Self {
            green_icon: green,
            red_icon: red,
        })
    }
}

impl Drop for DynamicIcons {
    fn drop(&mut self) {
        unsafe {
            if !self.green_icon.is_null() {
                DestroyIcon(self.green_icon);
            }
            if !self.red_icon.is_null() {
                DestroyIcon(self.red_icon);
            }
        }
    }
}

fn create_status_icon(is_allowed: bool) -> Result<HICON, String> {
    const SIZE: usize = 32;
    let mut pixels = vec![0u32; SIZE * SIZE];

    let center_x = 15.5f32;
    let center_y = 15.5f32;
    let outer_r = 13.5f32;
    let border_r = 11.5f32;

    // Colors (0xAARRGGBB)
    let (bg_r, bg_g, bg_b) = if is_allowed {
        (46u8, 204u8, 113u8) // Green #2ECC71
    } else {
        (231u8, 76u8, 60u8) // Red #E74C3C
    };

    let (bd_r, bd_g, bd_b) = if is_allowed {
        (39u8, 174u8, 96u8) // Dark green #27AE60
    } else {
        (192u8, 57u8, 43u8) // Dark red #C0392B
    };

    for y in 0..SIZE {
        for x in 0..SIZE {
            let fx = x as f32;
            let fy = y as f32;
            let dx = fx - center_x;
            let dy = fy - center_y;
            let dist = (dx * dx + dy * dy).sqrt();

            if dist > outer_r {
                // Transparent
                pixels[y * SIZE + x] = 0;
                continue;
            }

            // Anti-aliased edge
            let edge_alpha = if dist > outer_r - 1.0 {
                ((outer_r - dist) * 255.0).clamp(0.0, 255.0) as u8
            } else {
                255u8
            };

            let (r, g, b) = if dist > border_r {
                (bd_r, bd_g, bd_b)
            } else {
                (bg_r, bg_g, bg_b)
            };

            // Premultiplied ARGB for Windows 32-bit DIB
            let pr = ((r as u32 * edge_alpha as u32) / 255) as u8;
            let pg = ((g as u32 * edge_alpha as u32) / 255) as u8;
            let pb = ((b as u32 * edge_alpha as u32) / 255) as u8;

            pixels[y * SIZE + x] = ((edge_alpha as u32) << 24)
                | ((pr as u32) << 16)
                | ((pg as u32) << 8)
                | (pb as u32);
        }
    }

    // Helper to draw anti-aliased line segment onto pixel buffer
    let mut draw_thick_line = |x0: f32, y0: f32, x1: f32, y1: f32, thickness: f32| {
        let length = ((x1 - x0).powi(2) + (y1 - y0).powi(2)).sqrt();
        let dx = (x1 - x0) / length;
        let dy = (y1 - y0) / length;

        for y in 0..SIZE {
            for x in 0..SIZE {
                let px = x as f32;
                let py = y as f32;

                let t = ((px - x0) * dx + (py - y0) * dy).clamp(0.0, length);
                let closest_x = x0 + t * dx;
                let closest_y = y0 + t * dy;
                let d = ((px - closest_x).powi(2) + (py - closest_y).powi(2)).sqrt();

                if d < thickness {
                    let alpha = ((1.0 - (d / thickness).powi(2)) * 255.0).clamp(0.0, 255.0) as u8;
                    if alpha > 40 {
                        let a_u32 = alpha as u32;
                        pixels[y * SIZE + x] = (a_u32 << 24) | (a_u32 << 16) | (a_u32 << 8) | a_u32;
                    }
                }
            }
        }
    };

    if is_allowed {
        // Draw crisp checkmark
        draw_thick_line(9.5, 15.5, 14.0, 20.0, 1.6);
        draw_thick_line(14.0, 20.0, 22.5, 10.5, 1.6);
    } else {
        // Draw crisp cross (X)
        draw_thick_line(11.0, 11.0, 20.0, 20.0, 1.6);
        draw_thick_line(11.0, 20.0, 20.0, 11.0, 1.6);
    }

    unsafe {
        let hdc = CreateCompatibleDC(null_mut());
        if hdc.is_null() {
            return Err("CreateCompatibleDC failed".to_string());
        }

        let bmi = BITMAPINFO {
            bmiHeader: BITMAPINFOHEADER {
                biSize: std::mem::size_of::<BITMAPINFOHEADER>() as u32,
                biWidth: SIZE as i32,
                biHeight: -(SIZE as i32), // top-down
                biPlanes: 1,
                biBitCount: 32,
                biCompression: BI_RGB,
                biSizeImage: (SIZE * SIZE * 4) as u32,
                biXPelsPerMeter: 0,
                biYPelsPerMeter: 0,
                biClrUsed: 0,
                biClrImportant: 0,
            },
            bmiColors: [RGBQUAD {
                rgbBlue: 0,
                rgbGreen: 0,
                rgbRed: 0,
                rgbReserved: 0,
            }],
        };

        let mut bits_ptr: *mut core::ffi::c_void = null_mut();
        let hbm_color = CreateDIBSection(
            hdc,
            &bmi,
            DIB_RGB_COLORS,
            &mut bits_ptr,
            null_mut(),
            0,
        );

        if hbm_color.is_null() || bits_ptr.is_null() {
            DeleteDC(hdc);
            return Err("CreateDIBSection failed".to_string());
        }

        std::ptr::copy_nonoverlapping(
            pixels.as_ptr() as *const u8,
            bits_ptr as *mut u8,
            SIZE * SIZE * 4,
        );

        // Monochrome mask bitmap (all 0 for transparent)
        let mask_bytes = vec![0u8; (SIZE * SIZE) / 8];
        let hbm_mask: HBITMAP = CreateBitmap(
            SIZE as i32,
            SIZE as i32,
            1,
            1,
            mask_bytes.as_ptr() as *const _,
        );

        let icon_info = ICONINFO {
            fIcon: 1,
            xHotspot: 0,
            yHotspot: 0,
            hbmMask: hbm_mask,
            hbmColor: hbm_color,
        };

        let hicon = CreateIconIndirect(&icon_info);

        DeleteObject(hbm_color);
        DeleteObject(hbm_mask);
        DeleteDC(hdc);

        if hicon.is_null() {
            Err("CreateIconIndirect failed".to_string())
        } else {
            Ok(hicon)
        }
    }
}
