use bevy::ecs::reflect::ReflectComponent;
use bevy::math::Mat4;
use bevy::reflect::Reflect;
use bevy::render::camera::{CameraProjection, DepthCalculation};

use bevy_openxr_core::XrFovf;

#[derive(Debug, Clone)]
pub struct XRProjection {
    pub near: f32,
    pub far: f32,
    pub fov: Option<f32>,
}

impl XRProjection {
    pub fn new(near: f32, far: f32) -> Self {
        XRProjection {
            near,
            far,
            fov: None,
        }
    }
}

impl CameraProjection for XRProjection {
    fn get_projection_matrix(&self) -> Mat4 {
        panic!("XRProjection.get_projection_matrix() called. Need to call get_projection_matrix_fov(fov)")
    }

    fn update(&mut self, _width: f32, _height: f32) {}

    fn depth_calculation(&self) -> DepthCalculation {
        DepthCalculation::Distance
    }

    fn get_fov(&self) -> f32 {
        self.fov.unwrap_or(0.0)
    }

    fn get_near(&self) -> f32 {
        self.near
    }

    fn get_far(&self) -> f32 {
        self.far
    }
}

impl Default for XRProjection {
    fn default() -> Self {
        XRProjection {
            near: 0.05,
            far: 1000.,
            fov: None,
        }
    }
}

impl XRProjection {
    // =============================================================================
    // math code adapted from
    // https://github.com/KhronosGroup/OpenXR-SDK-Source/blob/master/src/common/xr_linear.h
    // Copyright (c) 2017 The Khronos Group Inc.
    // Copyright (c) 2016 Oculus VR, LLC.
    // SPDX-License-Identifier: Apache-2.0
    // =============================================================================
    pub fn get_projection_matrix_fov(&mut self, fov: &XrFovf) -> Mat4 {
        self.fov = Some(fov.angle_right.abs() + fov.angle_left.abs()); // TODO ok?

        let is_vulkan_api = false; // FIXME wgpu probably abstracts this
        let near_z = self.near;
        let far_z = self.far;

        let tan_angle_left = fov.angle_left.tan();
        let tan_angle_right = fov.angle_right.tan();

        let tan_angle_down = fov.angle_down.tan();
        let tan_angle_up = fov.angle_up.tan();

        let tan_angle_width = tan_angle_right - tan_angle_left;

        // Set to tanAngleDown - tanAngleUp for a clip space with positive Y
        // down (Vulkan). Set to tanAngleUp - tanAngleDown for a clip space with
        // positive Y up (OpenGL / D3D / Metal).
        // const float tanAngleHeight =
        //     graphicsApi == GRAPHICS_VULKAN ? (tanAngleDown - tanAngleUp) : (tanAngleUp - tanAngleDown);
        let tan_angle_height = if is_vulkan_api {
            tan_angle_down - tan_angle_up
        } else {
            tan_angle_up - tan_angle_down
        };

        // Set to nearZ for a [-1,1] Z clip space (OpenGL / OpenGL ES).
        // Set to zero for a [0,1] Z clip space (Vulkan / D3D / Metal).
        // const float offsetZ =
        //     (graphicsApi == GRAPHICS_OPENGL || graphicsApi == GRAPHICS_OPENGL_ES) ? nearZ : 0;
        // FIXME handle enum of graphics apis
        let offset_z = if !is_vulkan_api { near_z } else { 0. };

        let mut cols: [f32; 16] = [0.0; 16];

        if far_z <= near_z {
            // place the far plane at infinity
            cols[0] = 2. / tan_angle_width;
            cols[4] = 0.;
            cols[8] = (tan_angle_right + tan_angle_left) / tan_angle_width;
            cols[12] = 0.;

            cols[1] = 0.;
            cols[5] = 2. / tan_angle_height;
            cols[9] = (tan_angle_up + tan_angle_down) / tan_angle_height;
            cols[13] = 0.;

            cols[2] = 0.;
            cols[6] = 0.;
            cols[10] = -1.;
            cols[14] = -(near_z + offset_z);

            cols[3] = 0.;
            cols[7] = 0.;
            cols[11] = -1.;
            cols[15] = 0.;
        } else {
            // normal projection
            cols[0] = 2. / tan_angle_width;
            cols[4] = 0.;
            cols[8] = (tan_angle_right + tan_angle_left) / tan_angle_width;
            cols[12] = 0.;

            cols[1] = 0.;
            cols[5] = 2. / tan_angle_height;
            cols[9] = (tan_angle_up + tan_angle_down) / tan_angle_height;
            cols[13] = 0.;

            cols[2] = 0.;
            cols[6] = 0.;
            cols[10] = -(far_z + offset_z) / (far_z - near_z);
            cols[14] = -(far_z * (near_z + offset_z)) / (far_z - near_z);

            cols[3] = 0.;
            cols[7] = 0.;
            cols[11] = -1.;
            cols[15] = 0.;
        }

        Mat4::from_cols_array(&cols)
    }
}

// https://gitlab.freedesktop.org/monado/demos/openxr-simple-example/-/blob/master/main.c#L70

#[cfg(test)]
mod tests {
    use super::*;
    use bevy::math::Vec4;

    #[test]
    fn test_projection() {
        let projection = XRProjection::new(0.01, 100.);

        let matrix = projection.get_projection_matrix_fov(&XrFovf {
            angle_left: -0.8552113,
            angle_right: 0.7853982,
            angle_up: 0.83775806,
            angle_down: -0.87266463,
        });

        // FIXME approx tests?
        assert_eq!(
            matrix,
            Mat4::from_cols(
                Vec4::new(0.93007326, 0.0, 0.0, 0.0),
                Vec4::new(0.0, 0.86867154, 0.0, 0.0),
                Vec4::new(-0.06992678, -0.035242435, -1.0002, -1.0),
                Vec4::new(0.0, 0.0, -0.020002, 0.0),
            )
        );
    }
}
