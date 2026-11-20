/// Numeric block IDs, stored as bytes; unknown IDs remain representable.
pub struct Block;

impl Block {
    pub const AIR: u8 = 0;
    pub const STONE: u8 = 1;
    pub const DIRT: u8 = 2;
    pub const GRASS: u8 = 3;
    pub const SAND: u8 = 4;
    pub const SNOW: u8 = 5;
    pub const WATER: u8 = 6;
    pub const LOG: u8 = 7;
    pub const LEAVES: u8 = 8;
    pub const PLANKS: u8 = 9;
    pub const GLASS: u8 = 10;
    pub const BRICK: u8 = 11;
    pub const COAL_ORE: u8 = 12;
    pub const IRON_ORE: u8 = 13;
    pub const GOLD_ORE: u8 = 14;
    pub const DIAMOND_ORE: u8 = 15;
    pub const GRAVEL: u8 = 16;
    pub const ICE: u8 = 17;
    pub const CACTUS: u8 = 18;
    pub const BEDROCK: u8 = 19;
    pub const WOOL_RED: u8 = 20;
    pub const WOOL_GREEN: u8 = 21;
    pub const WOOL_BLUE: u8 = 22;
    pub const GLOWSTONE: u8 = 23;
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum RenderMode {
    Opaque,
    Cutout,
    Blend,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct BlockFaces {
    pub top: &'static str,
    pub side: &'static str,
    pub bottom: &'static str,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct BlockDef {
    pub id: u8,
    pub name: &'static str,
    pub faces: BlockFaces,
    pub render_mode: RenderMode,
    pub hides_neighbor_faces: bool,
    pub casts_ao: bool,
    pub collidable: bool,
    pub fluid: bool,
    pub sway: bool,
    pub light: u8,
    pub indestructible: bool,
}

const fn def(id: u8, name: &'static str, all: &'static str) -> BlockDef {
    BlockDef {
        id,
        name,
        faces: BlockFaces {
            top: all,
            side: all,
            bottom: all,
        },
        render_mode: RenderMode::Opaque,
        hides_neighbor_faces: true,
        casts_ao: true,
        collidable: true,
        fluid: false,
        sway: false,
        light: 0,
        indestructible: false,
    }
}

const DEFS: [BlockDef; 23] = [
    def(Block::STONE, "Stone", "stone"),
    def(Block::DIRT, "Dirt", "dirt"),
    BlockDef {
        faces: BlockFaces {
            top: "grass_top",
            side: "dirt_grass",
            bottom: "dirt",
        },
        ..def(Block::GRASS, "Grass", "stone")
    },
    def(Block::SAND, "Sand", "sand"),
    BlockDef {
        faces: BlockFaces {
            top: "snow",
            side: "dirt_snow",
            bottom: "dirt",
        },
        ..def(Block::SNOW, "Snow", "stone")
    },
    BlockDef {
        render_mode: RenderMode::Blend,
        hides_neighbor_faces: false,
        casts_ao: false,
        collidable: false,
        fluid: true,
        ..def(Block::WATER, "Water", "water")
    },
    BlockDef {
        faces: BlockFaces {
            top: "trunk_top",
            side: "trunk_side",
            bottom: "trunk_top",
        },
        ..def(Block::LOG, "Wood Log", "stone")
    },
    BlockDef {
        render_mode: RenderMode::Cutout,
        hides_neighbor_faces: false,
        casts_ao: false,
        sway: true,
        ..def(Block::LEAVES, "Leaves", "leaves_transparent")
    },
    def(Block::PLANKS, "Planks", "wood"),
    BlockDef {
        render_mode: RenderMode::Blend,
        hides_neighbor_faces: false,
        casts_ao: false,
        ..def(Block::GLASS, "Glass", "glass")
    },
    def(Block::BRICK, "Brick", "brick_red"),
    def(Block::COAL_ORE, "Coal Ore", "stone_coal"),
    def(Block::IRON_ORE, "Iron Ore", "stone_iron"),
    def(Block::GOLD_ORE, "Gold Ore", "stone_gold"),
    def(Block::DIAMOND_ORE, "Diamond Ore", "stone_diamond"),
    def(Block::GRAVEL, "Gravel", "gravel_stone"),
    BlockDef {
        render_mode: RenderMode::Blend,
        hides_neighbor_faces: false,
        casts_ao: false,
        ..def(Block::ICE, "Ice", "ice")
    },
    BlockDef {
        faces: BlockFaces {
            top: "cactus_top",
            side: "cactus_side",
            bottom: "cactus_top",
        },
        render_mode: RenderMode::Cutout,
        hides_neighbor_faces: false,
        ..def(Block::CACTUS, "Cactus", "stone")
    },
    BlockDef {
        indestructible: true,
        ..def(Block::BEDROCK, "Bedrock", "greystone")
    },
    def(Block::WOOL_RED, "Red Wool", "cotton_red"),
    def(Block::WOOL_GREEN, "Green Wool", "cotton_green"),
    def(Block::WOOL_BLUE, "Blue Wool", "cotton_blue"),
    BlockDef {
        light: 15,
        ..def(Block::GLOWSTONE, "Glowstone", "lava")
    },
];

pub fn block_def(id: u8) -> Option<&'static BlockDef> {
    if id == Block::AIR {
        return None;
    }
    DEFS.get(usize::from(id) - 1)
}

/// Representative particle tint, retaining the TypeScript number precision.
pub fn block_color(id: u8) -> [f64; 3] {
    match id {
        Block::STONE => [0.5, 0.5, 0.5],
        Block::DIRT => [0.45, 0.32, 0.2],
        Block::GRASS => [0.35, 0.6, 0.25],
        Block::SAND => [0.85, 0.78, 0.5],
        Block::SNOW => [0.92, 0.95, 0.98],
        Block::WATER => [0.25, 0.45, 0.85],
        Block::LOG => [0.45, 0.33, 0.2],
        Block::LEAVES => [0.25, 0.5, 0.2],
        Block::PLANKS => [0.7, 0.55, 0.35],
        Block::GLASS => [0.7, 0.85, 0.9],
        Block::BRICK => [0.7, 0.3, 0.25],
        Block::COAL_ORE => [0.3, 0.3, 0.3],
        Block::IRON_ORE => [0.7, 0.6, 0.5],
        Block::GOLD_ORE => [0.85, 0.75, 0.35],
        Block::DIAMOND_ORE => [0.5, 0.8, 0.85],
        Block::GRAVEL => [0.5, 0.48, 0.46],
        Block::ICE => [0.7, 0.85, 0.95],
        Block::CACTUS => [0.3, 0.5, 0.25],
        Block::BEDROCK => [0.25, 0.25, 0.25],
        Block::WOOL_RED => [0.8, 0.2, 0.2],
        Block::WOOL_GREEN => [0.3, 0.7, 0.3],
        Block::WOOL_BLUE => [0.25, 0.4, 0.8],
        Block::GLOWSTONE => [1.0, 0.78, 0.4],
        _ => [0.6, 0.6, 0.6],
    }
}

pub fn is_air(id: u8) -> bool {
    id == Block::AIR
}

pub fn is_indestructible(id: u8) -> bool {
    block_def(id).is_some_and(|d| d.indestructible)
}

pub fn block_light(id: u8) -> u8 {
    block_def(id).map_or(0, |d| d.light)
}

pub fn light_opaque(id: u8) -> bool {
    block_def(id).is_some_and(|d| d.hides_neighbor_faces)
}

/// Tile order matches numeric DEFS keys and JavaScript Set insertion order.
pub fn all_referenced_tiles() -> Vec<&'static str> {
    let mut tiles = Vec::new();
    for d in &DEFS {
        for tile in [d.faces.top, d.faces.side, d.faces.bottom] {
            if !tiles.contains(&tile) {
                tiles.push(tile);
            }
        }
    }
    tiles
}

pub const HOTBAR: [u8; 10] = [
    Block::GRASS,
    Block::DIRT,
    Block::STONE,
    Block::SAND,
    Block::LOG,
    Block::PLANKS,
    Block::LEAVES,
    Block::GLASS,
    Block::BRICK,
    Block::GLOWSTONE,
];
