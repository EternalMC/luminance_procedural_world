//! Module related to managing and drawing terrain.

mod mesh_gen;
mod voxel;
mod world_gen;

use std::collections::HashMap;
use std::mem;
use std::sync::{Arc, Mutex};
use std::sync::mpsc::{self, Receiver, Sender};
use std::thread;
use std::time::{Duration, Instant};
use luminance::framebuffer::Framebuffer;
use luminance::linear::M44;
use luminance::pipeline::{entry, pipeline, RenderState};
use luminance::texture::{Dim2, Flat};
use luminance::shader::program::{Program, ProgramError, Uniform, UniformBuilder,
                                 UniformInterface, UniformWarning};
use luminance_glfw::{Device, GLFWDevice};
use camera::Camera;
use maths::{Frustum, ToMatrix, Translation};
use model::Drawable;
use resources::Resources;
use shader;
use self::voxel::{AdjacentSectors, BlockList, Sector};
use self::world_gen::WorldGen;

// Type of terrain position vertex attribute.
type Position = [f32; 3];

// Type of terrain texture coordinate attribute.
type UV = [f32; 2];

// Type of face attribute. Serves to replace the normal
// vector, since on a cube the normals always lie along
// an axis.
type FaceNum = u32;

// A terrain vertex.
type Vertex = (Position, UV, FaceNum);

/// The length of one side of a cubic sector.
pub const SECTOR_SIZE: usize = 32;

const CLEAR_COLOR: [f32; 4] = [0.2, 0.75, 0.8, 1.0];

/// Drawable manager for world terrain. Handles the rendering
/// of each sector.
pub struct Terrain<'a> {
    shader: Program<Vertex, (), Uniforms>,
    resources: &'a Resources,
    sectors: HashMap<(i32, i32, i32), Sector>,
    shared_info: SharedInfo,
    nearby_rx: Receiver<Nearby>,
    needed_tx: Sender<(i32, i32, i32)>,
}

impl<'a> Terrain<'a> {
    /// Create a new `Terrain` using the shared `Resources`.
    /// # Panics
    /// This constructor panics if shaders fail to load.
    pub fn new(resources: &'a Resources) -> Terrain<'a> {
        let (shader, warnings) = Self::load_shaders().unwrap();
        for warn in &warnings {
            eprintln!("{:?}", warn);
        }
        
        let shared_info = Arc::new(Mutex::new(Default::default()));
        
        let sectors = HashMap::with_capacity(5 * 5 * 5);
        //for dx in -2..3 {
        //    for dy in -2..3 {                
        //        for dz in -2..3 {
        //            let pos = (dx, dy, dz);
        //            
        //            sectors.insert(pos, Sector::new(resources, pos));
        //            
        //            //println!("pos: {:?}", pos);
        //        }
        //    }
        //}
        
        //sectors.insert((0, 0, 0), Sector::new(resources, (0, 0, 0), BlockList::new([Block::Loam; SECTOR_SIZE * SECTOR_SIZE * SECTOR_SIZE])));
        //sectors.insert((1, 0, 0), Sector::new(resources, (1, 0, 0), BlockList::new([Block::Loam; SECTOR_SIZE * SECTOR_SIZE * SECTOR_SIZE])));
        //sectors.insert((0, 0, 1), Sector::new(resources, (0, 0, 1), BlockList::new([Block::Loam; SECTOR_SIZE * SECTOR_SIZE * SECTOR_SIZE])));
        //sectors.insert((1, 0, 1), Sector::new(resources, (1, 0, 1), BlockList::new([Block::Loam; SECTOR_SIZE * SECTOR_SIZE * SECTOR_SIZE])));
        
        let (nearby_tx, nearby_rx) = mpsc::channel();
        let (needed_tx, needed_rx) = mpsc::channel();
        TerrainGenThread::new(shared_info.clone(), nearby_tx, needed_rx).spawn();
        
        Terrain {
            resources,
            sectors,
            shader,
            shared_info,
            nearby_rx,
            needed_tx,
        }
    }
    
    /// Perform a frame update.
    /// May block for some time until a mutex can be aquired.
    pub fn update(&mut self, camera: &Camera) {
        let translation = camera.translation().clone();
        self.shared_info.lock().unwrap().player_pos = translation.clone();
        
        let begin = Instant::now();
        while let Ok(nearby) = self.nearby_rx.try_recv() {
            match nearby {
                Nearby::Query { sector: sector_coords, should_render } => {
                    //println!("sector_coords: {:?} => {}", sector_coords, should_render);
                    
                    if self.sectors.contains_key(&sector_coords) {
                        if !should_render {
                            //println!("bail1");
                            break;
                        }
                        
                        let model;
                        {
                            let sector = self.sectors.get(&sector_coords).unwrap();
                            if !sector.blocks().needs_rendering() || sector.model().is_some() {
                                //println!("bail2");
                                break;
                            }
                            
                            //println!("sector_coords: {:?}", sector_coords);
                            
                            let back   = (sector_coords.0,     sector_coords.1,     sector_coords.2 - 1);
                            let front  = (sector_coords.0,     sector_coords.1,     sector_coords.2 + 1);
                            let top    = (sector_coords.0,     sector_coords.1 + 1, sector_coords.2    );
                            let bottom = (sector_coords.0,     sector_coords.1 - 1, sector_coords.2    );
                            let left   = (sector_coords.0 - 1, sector_coords.1,     sector_coords.2    );
                            let right  = (sector_coords.0 + 1, sector_coords.1,     sector_coords.2    );
                            
                            let back = self.sectors.get(&back);
                            if back.is_none() {
                                break;
                            }
                            
                            let front = self.sectors.get(&front);
                            if front.is_none() {
                                break;
                            }
                            
                            let top = self.sectors.get(&top);
                            if top.is_none() {
                                break;
                            }
                            
                            let bottom = self.sectors.get(&bottom);
                            if bottom.is_none() {
                                break;
                            }
                            
                            let left = self.sectors.get(&left);
                            if left.is_none() {
                                break;
                            }
                            
                            let right = self.sectors.get(&right);
                            if right.is_none() {
                                break;
                            }
                            
                            let adjacent = AdjacentSectors::new(back.unwrap(), front.unwrap(),
                                                                top.unwrap(), bottom.unwrap(),
                                                                left.unwrap(), right.unwrap());
                                
                            model = sector.create_model(self.resources, sector_coords, &adjacent);
                        }
                        
                        let sector = self.sectors.get_mut(&sector_coords).unwrap();
                        sector.set_model(model);
                    } else {
                        self.needed_tx.send(sector_coords).unwrap();
                    }
                },
                Nearby::Generated(sector_coords, block_list) => {
                    self.sectors.entry(sector_coords).or_insert_with(|| Sector::new(block_list));
                },
            }
            //println!("nearby: {:?}", sector);
            
            let duration = Instant::now() - begin;
            
            let seconds = duration.as_secs() as f64 +
                          duration.subsec_nanos() as f64 * 1e-9;
            
            if seconds > 0.05 {
                //println!("too long: {}", seconds);
                break;
            }
        }
        //println!("time: {:?}", Instant::now() - begin);
        
        let sector = sector_at(&translation);
        self.sectors.retain(|&k, _| {
            let dx = k.0 as f32 - sector.0 as f32;
            let dy = k.1 as f32 - sector.1 as f32;
            let dz = k.2 as f32 - sector.2 as f32;
            
            let dist_sq = dx * dx + dy * dy + dz * dz;
            
            //println!("{}", dist_sq);
            
            dist_sq < 280.
        });
    }
    
    fn load_shaders() ->
            Result<(Program<Vertex, (), Uniforms>, Vec<UniformWarning>), ProgramError> {
        
        let (vs, fs) = shader::load_shader_text("vs", "fs");
        
        Program::from_strings(None, &vs, None, &fs)
    }
}

impl<'a> Drawable for Terrain<'a> {
    //type Vertex = TerrainVertex;
    //type Uniform = TerrainUniforms;
    
    fn draw(&self,
            device: &mut GLFWDevice,
            render_target: &Framebuffer<Flat, Dim2, (), ()>,
            //shader: &Program<Self::Vertex, (), Self::Uniform>,
            camera: &Camera) {
        let frustum = camera.frustum();
        
        device.draw(|| {
            entry(|gpu| {                    
                // TODO: Only bind the texture once, and ensure
                // that the correct one is used.
                pipeline(render_target, CLEAR_COLOR, |shade_gate| {
                    //let mut skipped = 0;
                    //let mut air = 0;
                    
                    for i in &self.sectors {
                        if let Some(model) = i.1.model() {
                            if !sector_visible(&frustum, *i.0) {
                                //skipped += 1;
                                continue;
                            }
                            
                            gpu.bind_texture(&model.tex.0);
                            shade_gate.shade(&self.shader, |render_gate, uniforms| {
                                uniforms.model_matrix.update(model.to_matrix());
                                uniforms.view_matrix.update(camera.to_matrix());
                                uniforms.projection_matrix.update(*camera.projection_matrix());
                                //uniforms.terrain_tex.update(bound);
                                
                                let render_state = RenderState::default();
                                                   //.set_face_culling(None);
                                render_gate.render(render_state, |tess_gate| {
                                    tess_gate.render((&model.tess).into());
                                });
                            });
                        }/* else {
                            air += 1;
                        }*/
                    }
                    
                    //println!("skipped: {} / {})", skipped, self.sectors.len() - air);
                });
            });
        });
    }
}

/// Terrain's uniform interface.
struct Uniforms {
    // Model transform.
    model_matrix: Uniform<M44>,
    
    // Camera view.
    view_matrix: Uniform<M44>,
    
    // 3D Projection.
    projection_matrix: Uniform<M44>,
    
    // Terrain Texture Atlas.
    //pub terrain_tex: Uniform<BoundTexture<'a, Texture<Flat, Dim2, RGB8UI>>>,
}

impl<'a> UniformInterface for Uniforms {
    fn uniform_interface(builder: UniformBuilder)
            -> Result<(Uniforms, Vec<UniformWarning>), ProgramError> {
        
        let model_matrix = builder.ask("model_matrix").unwrap();
        let view_matrix = builder.ask("view_matrix").unwrap();
        let projection_matrix = builder.ask("projection_matrix").unwrap();
        //let terrain_tex = builder.ask("terrain_tex").unwrap();
        
        Ok((Uniforms {
            model_matrix,
            view_matrix,
            projection_matrix,
            //terrain_tex,
        }, Vec::new()))
    }
}

// Information shared between the main thread
// and the worldgen thread.
#[derive(Debug)]
struct WorldGenThreadInfo {
     player_pos: Translation,
}

type SharedInfo = Arc<Mutex<WorldGenThreadInfo>>;

impl Default for WorldGenThreadInfo {
    fn default() -> WorldGenThreadInfo {
        WorldGenThreadInfo {
            player_pos: Translation::new(0., 0., 0.),
        }
    }
}

// Type for the 'nearby sector' channel.
enum Nearby {
    Query {
        sector: (i32, i32, i32),
        should_render: bool,
    },
    Generated((i32, i32, i32), BlockList),
}

const GENERATE_ORDER: [i32; 7] = [0, -1, 1, -2, 2, 3, -3];
const RENDER_DIST_AXIS: i32 = 2;

struct TerrainGenThread {
    shared_info: SharedInfo,
    nearby_tx: Sender<Nearby>,
    needed_rx: Receiver<(i32, i32, i32)>,
    gen: WorldGen,
}

impl TerrainGenThread {
    fn new(shared_info: SharedInfo,
           nearby_tx: Sender<Nearby>,
           needed_rx: Receiver<(i32, i32, i32)>) -> TerrainGenThread {
        TerrainGenThread {
            shared_info,
            nearby_tx,
            needed_rx,
            gen: WorldGen::new(),
        }
    }
    
    fn spawn(self) {
        thread::spawn(move || {
            loop {                
                let info = self.shared_info.lock().unwrap();
                let player_pos = info.player_pos.clone();
                //println!("{:?}", player_pos);
                mem::drop(info);
                
                let sector = sector_at(&player_pos);
                //println!("{:?}", sector);
                
                for dx in &GENERATE_ORDER {
                    for dy in -2..3 {
                        for dz in &GENERATE_ORDER {
                            let sector = (sector.0 + dx,
                                          sector.1 + dy,
                                          sector.2 + dz);
                            
                            let should_render = dx.abs() <= RENDER_DIST_AXIS &&
                                                dy.abs() <= 1 &&
                                                dz.abs() <= RENDER_DIST_AXIS;
                            
                            if self.nearby_tx.send(Nearby::Query { sector, should_render }).is_err() {
                                return;
                            }
                            
                            //println!("should_render: {}", should_render);
                            
                            /*
                            if dx.abs() <= RENDER_DIST_AXIS && dz.abs() <= RENDER_DIST_AXIS {
                                
                            } else {
                                println!("won't render {:?}", sector);
                            }
                            */
                        }
                    }
                }
                
                //
                
                while let Ok(needed) = self.needed_rx.try_recv() {
                    //println!("will generate: {:?}", needed);
                    
                    let list = self.gen.generate(needed);
                    
                    if self.nearby_tx.send(Nearby::Generated(needed, list)).is_err() {
                        return;
                    }
                }
                
                thread::sleep(Duration::from_secs(3));
                //println!("tick");
            }
        });
    }
}

// The nearest sector at a specific position.
fn sector_at(pos: &Translation) -> (i32, i32, i32) {
    ((pos.x.round() / SECTOR_SIZE as f32).floor() as i32,
     (pos.y.round() / SECTOR_SIZE as f32).floor() as i32,
     (pos.z.round() / SECTOR_SIZE as f32).floor() as i32)
}

const SECTOR_SIZE_F: f32 = SECTOR_SIZE as f32;
const SECTOR_SIZE_F_2: f32 = SECTOR_SIZE_F / 2.;

fn sector_visible(frustum: &Frustum, pos: (i32, i32, i32)) -> bool {
    // Convert sector coords to world space.
    let pos = (pos.0 as f32 * SECTOR_SIZE_F + SECTOR_SIZE_F_2,
               pos.1 as f32 * SECTOR_SIZE_F + SECTOR_SIZE_F_2,
               pos.2 as f32 * SECTOR_SIZE_F + SECTOR_SIZE_F_2);
    
    //println!("pos: {:?}", pos);
    //true
    
    for i in frustum.planes() {
        //println!("plane: {:?}", i);
        
        let d = i.a * pos.0 + i.b * pos.1 + i.c * pos.2 + i.d;
        
        if d <= -SECTOR_SIZE_F {
            return false;
        }
    }
    
    true
}
