//! The first person camera is in this module.

use luminance::linear::M44;
use maths::{self, Rotation, ToMatrix, Translation};

pub struct Camera {
    pos: Translation,
    rot: Rotation,
}

impl Camera {
    /// Creates a camera centered at the origin (0, 0, 0).
    pub fn new() -> Camera {
        Camera {
            pos: Translation::new(0., 0., 0.,),
            rot: Rotation::new(0., 0.),
        }
    }
    
    /// Allows reading of the camera's translation.
    pub fn translation(&self) -> &Translation {
        &self.pos
    }
    
    /// Allows access to the camera's translation.
    pub fn translation_mut(&mut self) -> &mut Translation {
        &mut self.pos
    }
    
    /// Allows reading of the camera's rotation.
    pub fn rotation(&self) -> &Rotation {
        &self.rot
    }
    
    /// Allows access to the camera's translation.
    pub fn rotation_mut(&mut self) -> &mut Rotation {
        &mut self.rot
    }
    
    /// Move the camera based on the current direction.
    pub fn move_dir(&mut self, dir: MovementDirection, distance: f32) {
        use ::std::f32::consts::FRAC_PI_2;
        use self::MovementDirection::*;
        
        match dir {
            Forward => {
                self.pos.x -= distance * self.rot.y.sin();
                self.pos.z -= distance * self.rot.y.cos();
            },
            
            Backward => {
                self.pos.x += distance * self.rot.y.sin();
                self.pos.z += distance * self.rot.y.cos();
            },
            
            Left => {
                let ry = self.rot.y + FRAC_PI_2;
                
                self.pos.x -= distance * ry.sin();
                self.pos.z -= distance * ry.cos();
            },
            
            Right => {
                let ry = self.rot.y + FRAC_PI_2;
                
                self.pos.x += distance * ry.sin();
                self.pos.z += distance * ry.cos();
            }
        }
    }
}

impl ToMatrix for Camera {
    fn to_matrix(&self) -> M44 {
        let pos = Translation::new(-self.pos.x, -self.pos.y, -self.pos.z);
        let rot = Rotation::new(-self.rot.x, -self.rot.y);
        
        //let new_test = Translation::new(0.5, 0., 0.);
        //maths::matrix_mul(&new_test.to_matrix(), &pos.to_matrix())
        
        maths::matrix_mul(&rot.to_matrix(), &pos.to_matrix())
        //pos.to_matrix()
        //rot.to_matrix()
    }
}

/// Represents the direction of movement for the camera.
#[derive(Debug, Clone, Copy)]
pub enum MovementDirection {
    Forward,
    Backward,
    Left,
    Right,
}
