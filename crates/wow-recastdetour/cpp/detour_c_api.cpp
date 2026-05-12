#include "DetourNavMesh.h"
#include "DetourAlloc.h"
#include "DetourStatus.h"
#include "DetourNavMeshBuilder.h"
#include "DetourNavMeshQuery.h"

#include <stdint.h>
#include <string.h>

extern "C"
{
    dtNavMesh* rustycore_dt_alloc_nav_mesh()
    {
        return dtAllocNavMesh();
    }

    void rustycore_dt_free_nav_mesh(dtNavMesh* mesh)
    {
        dtFreeNavMesh(mesh);
    }

    dtStatus rustycore_dt_nav_mesh_init(dtNavMesh* mesh, dtNavMeshParams const* params)
    {
        return mesh->init(params);
    }

    uint32_t rustycore_dt_nav_mesh_get_max_tiles(dtNavMesh const* mesh)
    {
        return mesh->getMaxTiles();
    }

    dtStatus rustycore_dt_nav_mesh_add_tile_copy(
        dtNavMesh* mesh,
        unsigned char const* data,
        int data_size,
        int flags,
        uint64_t* result)
    {
        unsigned char* detour_data = (unsigned char*)dtAlloc(data_size, DT_ALLOC_PERM);
        if (!detour_data)
            return DT_FAILURE | DT_OUT_OF_MEMORY;

        memcpy(detour_data, data, data_size);
        dtTileRef tile_ref = 0;
        dtStatus status = mesh->addTile(detour_data, data_size, flags, 0, &tile_ref);
        if (dtStatusFailed(status))
        {
            dtFree(detour_data);
            return status;
        }

        if (result)
            *result = tile_ref;

        return status;
    }

    dtStatus rustycore_dt_nav_mesh_remove_tile(dtNavMesh* mesh, uint64_t tile_ref)
    {
        return mesh->removeTile((dtTileRef)tile_ref, 0, 0);
    }

    dtNavMeshQuery* rustycore_dt_alloc_nav_mesh_query()
    {
        return dtAllocNavMeshQuery();
    }

    void rustycore_dt_free_nav_mesh_query(dtNavMeshQuery* query)
    {
        dtFreeNavMeshQuery(query);
    }

    dtStatus rustycore_dt_nav_mesh_query_init(dtNavMeshQuery* query, dtNavMesh const* mesh, int max_nodes)
    {
        return query->init(mesh, max_nodes);
    }

    void rustycore_dt_free(void* ptr)
    {
        dtFree(ptr);
    }

    bool rustycore_dt_create_square_tile_data(int tile_x, int tile_y, unsigned char** out_data, int* out_data_size)
    {
        unsigned short verts[] = {
            0, 0, 0,
            1, 0, 0,
            1, 0, 1,
            0, 0, 1,
        };
        unsigned short polys[] = {
            0, 1, 2, 3,
            0, 0, 0, 0,
        };
        unsigned short poly_flags[] = { 1 };
        unsigned char poly_areas[] = { 0 };

        dtNavMeshCreateParams params;
        memset(&params, 0, sizeof(params));
        params.verts = verts;
        params.vertCount = 4;
        params.polys = polys;
        params.polyFlags = poly_flags;
        params.polyAreas = poly_areas;
        params.polyCount = 1;
        params.nvp = 4;
        params.tileX = tile_x;
        params.tileY = tile_y;
        params.tileLayer = 0;
        params.bmin[0] = 0.0f;
        params.bmin[1] = 0.0f;
        params.bmin[2] = 0.0f;
        params.bmax[0] = 1.0f;
        params.bmax[1] = 1.0f;
        params.bmax[2] = 1.0f;
        params.walkableHeight = 2.0f;
        params.walkableRadius = 0.0f;
        params.walkableClimb = 0.9f;
        params.cs = 1.0f;
        params.ch = 1.0f;
        params.buildBvTree = true;

        return dtCreateNavMeshData(&params, out_data, out_data_size);
    }
}
