//! A module for managing the voxels in the world.

use luminance::tess::{Mode, Tess, TessVertices};
use super::{Vertex, SECTOR_LEN, SECTOR_SIZE, SECTOR_SIZE_S};
use maths::Translation;
use model::Model;
use resources::Resources;

//const VERTICES: [Vertex; 3] = [
//  ([-0.5, -0.5, 0.0], [0.0, 1.0]),
//  ([-0.5,  0.5, 0.0], [0.0, 0.0]),
//  ([ 0.5, -0.5, 0.0], [1.0, 1.0]),
//];

/// A block in the world.
#[derive(Clone, Copy, Debug)]
pub enum Block {
    Air,
    Limestone,
    Loam,
    Grass,
    Tree,
    Leaves,
}

impl Block {
    /// Determine if the block is air.
    pub fn is_air(&self) -> bool {
        match *self {
            Block::Air => true,
            _ => false,
        }
    }
    
    /// Determine if the block must be drawn.
    pub fn needs_rendering(&self) -> bool {
        !self.is_air()
    }
}

/// The type of sector space coordinates.
#[derive(Clone, Copy, Debug)]
pub struct SectorSpaceCoords {
    x: isize,
    y: isize,
    z: isize,
}

impl SectorSpaceCoords {
    /// Create a new coordinate triple.
    /// # Panics
    /// Panics if any component is >= `SECTOR_SIZE`.
    pub fn new(x: isize, y: isize, z: isize) -> SectorSpaceCoords {
        if x < 0              || y < 0              || z < 0              ||
           x >= SECTOR_SIZE_S || y >= SECTOR_SIZE_S || z >= SECTOR_SIZE_S {
            panic!("SectorSpaceCoords out of range");
        }
        
        SectorSpaceCoords {
            x,
            y,
            z,
        }
    }
    
    /// If possible, create the coord for the block
    /// behind this one.
    pub fn back(&self) -> Option<SectorSpaceCoords> {
        if self.z > 0 {
            Some(Self::new(self.x, self.y, self.z - 1))
        } else {
            None
        }
    }
    
    /// If possible, create the coord for the block
    /// in front of this one.
    pub fn front(&self) -> Option<SectorSpaceCoords> {
        if self.z < SECTOR_SIZE_S - 1 {
            Some(Self::new(self.x, self.y, self.z + 1))
        } else {
            None
        }
    }
    
    /// If possible, create the coord for the block
    /// above this one.
    pub fn top(&self) -> Option<SectorSpaceCoords> {
        if self.y < SECTOR_SIZE_S - 1 {
            Some(Self::new(self.x, self.y + 1, self.z))
        } else {
            None
        }
    }
    
    /// If possible, create the coord for the block
    /// below this one.
    pub fn bottom(&self) -> Option<SectorSpaceCoords> {
        if self.y > 0 {
            Some(Self::new(self.x, self.y - 1, self.z))
        } else {
            None
        }
    }
    
    /// If possible, create the coord for the block
    /// to the left of this one.
    pub fn left(&self) -> Option<SectorSpaceCoords> {
        if self.x > 0 {
            Some(Self::new(self.x - 1, self.y, self.z))
        } else {
            None
        }
    }
    
    /// If possible, create the coord for the block
    /// to the right of this one.
    pub fn right(&self) -> Option<SectorSpaceCoords> {
        if self.x < SECTOR_SIZE_S - 1 {
            Some(Self::new(self.x + 1, self.y, self.z))
        } else {
            None
        }
    }
    
    pub fn x(&self) -> isize { self.x }
    pub fn y(&self) -> isize { self.y }
    pub fn z(&self) -> isize { self.z }
}

/// The array structure of blocks in a `Sector`.
pub struct BlockList([Block; SECTOR_LEN]);

impl BlockList {
    /// Create a new `BlockList`, consuming the array
    /// of `Block`s.
    pub fn new(blocks: [Block; SECTOR_LEN]) -> BlockList {
        BlockList(blocks)
    }
    
    /// Create a new `BlockList` fulled with air.
    pub fn new_air() -> BlockList {
        BlockList([Block::Air; SECTOR_LEN])
    }

    /// Look at the block at a specific position in sector coords.
    pub fn get(&self, pos: SectorSpaceCoords) -> &Block {
        &self.0[Self::index(pos)]
    }
    
    /// Set a block at a specific position in sector coords.
    pub fn set(&mut self, pos: SectorSpaceCoords, block: Block) {
        self.0[Self::index(pos)] = block;
    }
    
    /// Determine if all blocks in the `BlockList` are air.
    pub fn needs_rendering(&self) -> bool {
        for i in self.0.iter() {
            if i.needs_rendering() {
                return true;
            }
        }
        
        false
    }
    
    // Determines the internal index of sector coords.
    fn index(pos: SectorSpaceCoords) -> usize {        
        let (x, y, z) = (pos.x() as usize, pos.y() as usize, pos.z() as usize);
        
        x + y * SECTOR_SIZE + z * SECTOR_SIZE * SECTOR_SIZE
    }
}

/// An iterator over a BlockList.
pub struct BlockListIter<'a> {
    //inner: iter::Enumerate<slice::Iter<'a, Block>>,
    list: &'a BlockList,
    x: isize,
    y: isize,
    z: isize,
}

type BlockListIterItem<'a> = (SectorSpaceCoords, &'a Block);

impl<'a> Iterator for BlockListIter<'a> {
    type Item = BlockListIterItem<'a>;
    
    fn next(&mut self) -> Option<Self::Item> {
        /*
        match self.0.next() {
            Some(i) => {
                let mut total = i.0;
                
                let z = total / (SECTOR_SIZE * SECTOR_SIZE);
                total -= z * SECTOR_SIZE * SECTOR_SIZE;
                let z = z as u8;
                
                let y = total / SECTOR_SIZE;
                total -= y * SECTOR_SIZE;
                let y = y as u8;
                
                let x = total;
                let x = x as u8;
                
                //println!("x: {}, y: {}, z: {}", x, y, z);
                
                Some((SectorSpaceCoords::new(x, y, z), i.1))
            },
            None => None,
        }
        */
        
        //println!("{} {} {}", self.x, self.y, self.z);
        
        if self.x + 1 < SECTOR_SIZE_S {
            self.x += 1;
        } else {
            self.x = 0;
            
            if self.y + 1 < SECTOR_SIZE_S {
                self.y += 1;
            } else {
                self.y = 0;
                
                if self.z + 1 < SECTOR_SIZE_S {
                    self.z += 1;
                } else {
                    return None;
                }
            }
        }
        
        let coords = SectorSpaceCoords::new(self.x, self.y, self.z);
        
        //println!("{:?}", coords);
        
        Some((coords, self.list.get(coords)))
    }
}

impl<'a> IntoIterator for &'a BlockList {
    type Item = BlockListIterItem<'a>;
    type IntoIter = BlockListIter<'a>;
    
    fn into_iter(self) -> BlockListIter<'a> {
        BlockListIter {
            list: self,
            x: -1, // Starts at 0, b/c next() increments x.
            y: 0,
            z: 0,
        }
    }
}

/// An individual "chunk" of the world.
pub struct Sector {
    blocks: BlockList,
    model: Option<Model<Vertex>>,
}

impl Sector {
    /// Create a sector.
    pub fn new(resources: &Resources, pos: (i32, i32, i32),
               blocks: BlockList, vertices: Vec<Vertex>) -> Sector {
        let model = if blocks.needs_rendering() {
            let terrain_tex = resources.terrain_tex();
            
            //let vertices = mesh_gen::generate_block_vertices(&blocks, &terrain_tex.1);
            let tess = Tess::new(Mode::Triangle, TessVertices::Fill(&vertices), None);
            
            let translation = Translation::new((pos.0 * SECTOR_SIZE as i32) as f32,
                                               (pos.1 * SECTOR_SIZE as i32) as f32,
                                               (pos.2 * SECTOR_SIZE as i32) as f32);
                                           
            //println!("translation: {:?}", translation);
            
            Some(Model::with_translation(tess, terrain_tex, translation))
        } else {
            None
        };

        Sector {
            blocks,
            model,
        }
    }
    
    /// Return an immutable reference to this sector's `Model`.
    /// The model may not exist, in which case `None` is returned.
    pub fn model(&self) -> Option<&Model<Vertex>> {
        self.model.as_ref()
    }
    
    /// Return this sector's `BlockList`.
    pub fn blocks(&self) -> &BlockList {
        &self.blocks
    }
}
