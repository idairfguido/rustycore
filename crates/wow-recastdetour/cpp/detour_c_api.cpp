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

    dtQueryFilter* rustycore_dt_alloc_query_filter()
    {
        return new dtQueryFilter();
    }

    void rustycore_dt_free_query_filter(dtQueryFilter* filter)
    {
        delete filter;
    }

    uint16_t rustycore_dt_query_filter_get_include_flags(dtQueryFilter const* filter)
    {
        return filter->getIncludeFlags();
    }

    void rustycore_dt_query_filter_set_include_flags(dtQueryFilter* filter, uint16_t flags)
    {
        filter->setIncludeFlags(flags);
    }

    uint16_t rustycore_dt_query_filter_get_exclude_flags(dtQueryFilter const* filter)
    {
        return filter->getExcludeFlags();
    }

    void rustycore_dt_query_filter_set_exclude_flags(dtQueryFilter* filter, uint16_t flags)
    {
        filter->setExcludeFlags(flags);
    }

    float rustycore_dt_query_filter_get_area_cost(dtQueryFilter const* filter, int area)
    {
        return filter->getAreaCost(area);
    }

    void rustycore_dt_query_filter_set_area_cost(dtQueryFilter* filter, int area, float cost)
    {
        filter->setAreaCost(area, cost);
    }

    dtStatus rustycore_dt_nav_mesh_query_find_nearest_poly(
        dtNavMeshQuery const* query,
        float const* center,
        float const* half_extents,
        dtQueryFilter const* filter,
        uint64_t* nearest_ref,
        float* nearest_point)
    {
        dtPolyRef poly_ref = 0;
        dtStatus status = query->findNearestPoly(center, half_extents, filter, &poly_ref, nearest_point);
        if (nearest_ref)
            *nearest_ref = poly_ref;

        return status;
    }

    dtStatus rustycore_dt_nav_mesh_query_find_path(
        dtNavMeshQuery const* query,
        uint64_t start_ref,
        uint64_t end_ref,
        float const* start_pos,
        float const* end_pos,
        dtQueryFilter const* filter,
        uint64_t* path,
        int* path_count,
        int max_path)
    {
        return query->findPath(
            (dtPolyRef)start_ref,
            (dtPolyRef)end_ref,
            start_pos,
            end_pos,
            filter,
            (dtPolyRef*)path,
            path_count,
            max_path);
    }

    dtStatus rustycore_dt_nav_mesh_query_find_straight_path(
        dtNavMeshQuery const* query,
        float const* start_pos,
        float const* end_pos,
        uint64_t const* path,
        int path_size,
        float* straight_path,
        unsigned char* straight_path_flags,
        uint64_t* straight_path_refs,
        int* straight_path_count,
        int max_straight_path,
        int options)
    {
        return query->findStraightPath(
            start_pos,
            end_pos,
            (dtPolyRef const*)path,
            path_size,
            straight_path,
            straight_path_flags,
            (dtPolyRef*)straight_path_refs,
            straight_path_count,
            max_straight_path,
            options);
    }

    dtStatus rustycore_dt_nav_mesh_query_closest_point_on_poly(
        dtNavMeshQuery const* query,
        uint64_t poly_ref,
        float const* position,
        float* closest,
        bool* position_over_poly)
    {
        return query->closestPointOnPoly((dtPolyRef)poly_ref, position, closest, position_over_poly);
    }

    dtStatus rustycore_dt_nav_mesh_query_closest_point_on_poly_boundary(
        dtNavMeshQuery const* query,
        uint64_t poly_ref,
        float const* position,
        float* closest)
    {
        return query->closestPointOnPolyBoundary((dtPolyRef)poly_ref, position, closest);
    }

    dtStatus rustycore_dt_nav_mesh_query_get_poly_height(
        dtNavMeshQuery const* query,
        uint64_t poly_ref,
        float const* position,
        float* height)
    {
        return query->getPolyHeight((dtPolyRef)poly_ref, position, height);
    }

    dtStatus rustycore_dt_nav_mesh_query_move_along_surface(
        dtNavMeshQuery const* query,
        uint64_t start_ref,
        float const* start_pos,
        float const* end_pos,
        dtQueryFilter const* filter,
        float* result_pos,
        uint64_t* visited,
        int* visited_count,
        int max_visited_size)
    {
        return query->moveAlongSurface(
            (dtPolyRef)start_ref,
            start_pos,
            end_pos,
            filter,
            result_pos,
            (dtPolyRef*)visited,
            visited_count,
            max_visited_size);
    }

    dtStatus rustycore_dt_nav_mesh_query_raycast(
        dtNavMeshQuery const* query,
        uint64_t start_ref,
        float const* start_pos,
        float const* end_pos,
        dtQueryFilter const* filter,
        float* hit_t,
        float* hit_normal,
        uint64_t* path,
        int* path_count,
        int max_path)
    {
        return query->raycast(
            (dtPolyRef)start_ref,
            start_pos,
            end_pos,
            filter,
            hit_t,
            hit_normal,
            (dtPolyRef*)path,
            path_count,
            max_path);
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
