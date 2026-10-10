//! All procedurally generated content: textures, meshes, models, clips.
//!
//! Meshes are stored CPU-side in upload order; the renderer must upload them
//! in the same order so `MeshHandle(i)` matches `meshes[i]`.

pub mod models;
pub mod props;
pub mod sounds;
pub mod textures;

use crate::anims;
use crate::rig;
use ashen_engine::anim::ClipLibrary;
use ashen_engine::math::Mat4;
use ashen_engine::mesh::{MeshData, rgb};
use ashen_engine::render::MeshHandle;
use ashen_engine::skeleton::Skeleton;
use ashen_engine::texture::TextureData;

#[derive(Default)]
pub struct AssetBuilder {
    pub meshes: Vec<MeshData>,
}

impl AssetBuilder {
    pub fn add(&mut self, m: MeshData) -> MeshHandle {
        self.meshes.push(m);
        MeshHandle(self.meshes.len() as u32 - 1)
    }
}

/// A rigid mesh attached to a bone.
#[derive(Clone, Copy, Debug)]
pub struct Part {
    pub bone: usize,
    pub mesh: MeshHandle,
    pub offset: Mat4,
}

impl Part {
    pub fn new(bone: usize, mesh: MeshHandle) -> Self {
        Part { bone, mesh, offset: Mat4::IDENTITY }
    }
}

#[derive(Clone, Debug)]
pub struct ModelDef {
    pub parts: Vec<Part>,
    /// Bounding radius around the model origin, for culling and lights.
    pub radius: f32,
}

pub struct ItemMeshes {
    pub longsword: MeshHandle,
    pub kite_shield: MeshHandle,
    pub broken_sword: MeshHandle,
    pub bow: MeshHandle,
    pub arrow: MeshHandle,
    pub flask: MeshHandle,
    pub club: MeshHandle,
}

pub struct PropMeshes {
    pub pyre: MeshHandle,
    pub urns: Vec<MeshHandle>,
    pub shard: MeshHandle,
    pub door: MeshHandle,
    pub big_door: MeshHandle,
    pub cell_bars: MeshHandle,
    pub portcullis: MeshHandle,
    pub boulder: MeshHandle,
    pub fog_small: MeshHandle,
    pub fog_big: MeshHandle,
    pub torch: MeshHandle,
    pub corpses: Vec<MeshHandle>,
    pub gravestones: Vec<MeshHandle>,
    pub rubble: MeshHandle,
    pub nest: MeshHandle,
    pub rune: MeshHandle,
    pub glint: MeshHandle,
    pub sky: MeshHandle,
    pub sky_dark: MeshHandle,
}

pub struct Assets {
    pub textures: Vec<TextureData>,
    pub meshes: Vec<MeshData>,
    pub human_skel: Skeleton,
    pub demon_skel: Skeleton,
    pub clips: ClipLibrary,
    pub knight: ModelDef,
    pub corvin: ModelDef,
    pub husk: ModelDef,
    pub husk_pale: ModelDef,
    pub demon: ModelDef,
    pub items: ItemMeshes,
    pub props: PropMeshes,
}

impl Assets {
    /// Generates everything except level geometry (appended later by the level builder).
    pub fn build() -> Assets {
        let textures = textures::generate_all();
        let mut ab = AssetBuilder::default();
        let knight = models::knight(&mut ab, models::KnightLook::PLAYER);
        let corvin = models::knight(&mut ab, models::KnightLook::CORVIN);
        let husk = models::husk(&mut ab, rgb(255, 255, 255));
        let husk_pale = models::husk(&mut ab, rgb(200, 205, 210));
        let demon = models::demon(&mut ab);
        let items = ItemMeshes {
            longsword: ab.add(props::longsword()),
            kite_shield: ab.add(props::kite_shield()),
            broken_sword: ab.add(props::broken_sword()),
            bow: ab.add(props::bow()),
            arrow: ab.add(props::arrow()),
            flask: ab.add(props::flask()),
            club: ab.add(props::demon_club()),
        };
        let props = PropMeshes {
            pyre: ab.add(props::pyre()),
            urns: (0..4).map(|i| ab.add(props::urn(i + 1))).collect(),
            shard: ab.add(props::shard()),
            door: ab.add(props::door_leaf(1.5, 2.6, 0.12, false)),
            big_door: ab.add(props::door_leaf(2.5, 6.0, 0.25, true)),
            cell_bars: ab.add(props::bars(1.6, 2.6)),
            portcullis: ab.add(props::bars(3.6, 4.0)),
            boulder: ab.add(props::boulder()),
            fog_small: ab.add(props::fog_veil(2.6, 3.0)),
            fog_big: ab.add(props::fog_veil(4.0, 5.0)),
            torch: ab.add(props::torch()),
            corpses: (0..3).map(|i| ab.add(props::corpse(i + 11))).collect(),
            gravestones: (0..4).map(|i| ab.add(props::gravestone(i + 21))).collect(),
            rubble: ab.add(props::rubble(31, 1.2)),
            nest: ab.add(props::nest()),
            rune: ab.add(props::floor_decal(1.1, textures::Tex::RuneRing, rgb(255, 160, 80))),
            glint: ab.add(props::item_glint()),
            sky: ab.add(props::sky_dome(rgb(70, 76, 86), rgb(150, 150, 146))),
            sky_dark: ab.add(props::sky_dome(rgb(20, 22, 26), rgb(60, 62, 64))),
        };
        let human_skel = rig::humanoid::skeleton();
        let demon_skel = rig::demon::skeleton();
        let mut clips = ClipLibrary::new();
        anims::humanoid::register(&mut clips, &human_skel);
        anims::demon::register(&mut clips, &demon_skel);
        Assets {
            textures,
            meshes: ab.meshes,
            human_skel,
            demon_skel,
            clips,
            knight,
            corvin,
            husk,
            husk_pale,
            demon,
            items,
            props,
        }
    }

    pub fn add_mesh(&mut self, m: MeshData) -> MeshHandle {
        self.meshes.push(m);
        MeshHandle(self.meshes.len() as u32 - 1)
    }
}
