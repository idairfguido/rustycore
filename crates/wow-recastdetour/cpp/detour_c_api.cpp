#include "DetourNavMesh.h"
#include "DetourAlloc.h"
#include "DetourStatus.h"

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
}
