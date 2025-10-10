use std::{io::Cursor, path::Path};

use asset_importer_rs_gltf::Gltf2Importer;
use asset_importer_rs_core::{AiImporter, AiImporterExt};
use asset_importer_rs_scene::AiMesh;

use crate::video::mesh_render_pipeline::Vertex;

// How should meshs work?
// 
// There are basically 2 things that a mesh could store, and perhaps both at once:
// 1) Its CPU-side buffer of vertex data
// 2) Its GPU-side buffer of vertex data
//
// In the case of (2), this could be a sub-slice of some buffer, and should also
// include the index buffer.
//
// For now, I'll just store the CPU side buffers, and figure out how I want the
// object graph in more detail later...
pub struct MeshData {
    pub vertex_data: Vec<Vertex>,
    pub index_data: Vec<u32>,
}

fn conv_vec3(vec3: asset_importer_rs_scene::AiVector3D) -> [f32; 3] {
    [vec3.x, vec3.y, vec3.z]
}

fn import_mesh(mesh: &AiMesh) -> MeshData {
    let mut vertex_data = vec![];
    let mut index_data: Vec<u32> = vec![];

    // TODO: How to process missing texture coordinates?
    let texcoords = mesh.texture_coords.get(0)
        .as_ref().unwrap().as_ref().unwrap();

    // If there is a second texcoords channel, use it, otherwise default back
    // to texcoords
    let texcoords_2 = match mesh.texture_coords.get(1).as_ref() {
        // Also fallback here
        Some(coords) => coords.as_ref().unwrap_or(texcoords),
        None => texcoords,
    };

    for i in 0..mesh.vertices.len() {
        let vertex = Vertex {
            position: conv_vec3(mesh.vertices[i]),
            normal:   conv_vec3(mesh.normals[i]),

            // For now, we have to manaully flip the UVs, as it doesn't necessarily
            // seem (?) like asset-importer-rs does it for us?
            uv:       [texcoords[i].x, 1.0 - texcoords[i].y],
            uv2:      [texcoords_2[i].x, 1.0 - texcoords_2[i].y],
        };

        vertex_data.push(vertex);
    }

    for face in &mesh.faces {
        assert!(face.len() == 3);
        for index in face {
            index_data.push(*index as u32);
        }
    }

    MeshData { vertex_data, index_data }
}

pub fn import_binary_data(data: &[u8]) -> Option<MeshData> {
    log::info!("begin asset import");
    
    let importer = Gltf2Importer::new();
    let scene = importer.read_file(Path::new("builtin.gltf"), |path| {
        Ok(Cursor::new(data))
    }).unwrap();

    // When using Russimp, we were using these flags:
    // vec![PostProcess::Triangulate, PostProcess::FlipUVs, PostProcess::GenerateUVCoords],
    //
    // Although, GenerateUVCoords may be unnecessary.
    //
    // It would be good to figure out the equivalent in the new library.

    for mesh in &scene.meshes {
        let mesh = Some(import_mesh(mesh));
        log::info!("finish asset import");
        return mesh;
    }

    None
}