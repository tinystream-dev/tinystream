// SPDX-License-Identifier: AGPL-3.0-or-later

// FFmpeg (and the desktop's mpv) is linked against this instead of libva, so
// tinystream starts on systems without it. Every call goes to the system's
// libva, opened on first use; without one, VA-API fails and transcoding (or
// decoding) falls back to software.

#include <va/va.h>
#include <va/va_str.h>
#include <va/va_vpp.h>

#include "shim.h"

static void *libva, *libva_drm;

const char *va_shim_error(void) {
    if (!shim_open(LIBVA, &libva) || !shim_open(LIBVA_DRM, &libva_drm))
        return dlerror();
    return NULL;
}

#define VA(ret, fail, name, params, args) FORWARD(LIBVA, libva, ret, fail, name, params, args)
#define STATUS(name, params, args) VA(VAStatus, VA_STATUS_ERROR_UNIMPLEMENTED, name, params, args)

VA(const char *, "unknown libva error", vaErrorStr, (VAStatus error_status), (error_status))
VA(const char *, "<unknown profile>", vaProfileStr, (VAProfile profile), (profile))
VA(const char *, "<unknown entrypoint>", vaEntrypointStr, (VAEntrypoint entrypoint), (entrypoint))
VA(const char *, NULL, vaQueryVendorString, (VADisplay dpy), (dpy))
VA(VAMessageCallback, NULL, vaSetErrorCallback, (VADisplay dpy, VAMessageCallback callback, void *user_context),
   (dpy, callback, user_context))
VA(VAMessageCallback, NULL, vaSetInfoCallback, (VADisplay dpy, VAMessageCallback callback, void *user_context),
   (dpy, callback, user_context))
VA(int, 0, vaMaxNumProfiles, (VADisplay dpy), (dpy))
VA(int, 0, vaMaxNumEntrypoints, (VADisplay dpy), (dpy))
VA(int, 0, vaMaxNumImageFormats, (VADisplay dpy), (dpy))

STATUS(vaInitialize, (VADisplay dpy, int *major_version, int *minor_version), (dpy, major_version, minor_version))
STATUS(vaTerminate, (VADisplay dpy), (dpy))
STATUS(vaSetDriverName, (VADisplay dpy, char *driver_name), (dpy, driver_name))
STATUS(vaGetDisplayAttributes, (VADisplay dpy, VADisplayAttribute *attr_list, int num_attributes),
       (dpy, attr_list, num_attributes))
STATUS(vaQueryConfigProfiles, (VADisplay dpy, VAProfile *profile_list, int *num_profiles),
       (dpy, profile_list, num_profiles))
STATUS(vaQueryConfigEntrypoints,
       (VADisplay dpy, VAProfile profile, VAEntrypoint *entrypoint_list, int *num_entrypoints),
       (dpy, profile, entrypoint_list, num_entrypoints))
STATUS(vaGetConfigAttributes,
       (VADisplay dpy, VAProfile profile, VAEntrypoint entrypoint, VAConfigAttrib *attrib_list, int num_attribs),
       (dpy, profile, entrypoint, attrib_list, num_attribs))
STATUS(vaCreateConfig,
       (VADisplay dpy, VAProfile profile, VAEntrypoint entrypoint, VAConfigAttrib *attrib_list, int num_attribs,
        VAConfigID *config_id),
       (dpy, profile, entrypoint, attrib_list, num_attribs, config_id))
STATUS(vaDestroyConfig, (VADisplay dpy, VAConfigID config_id), (dpy, config_id))
STATUS(vaQuerySurfaceAttributes,
       (VADisplay dpy, VAConfigID config, VASurfaceAttrib *attrib_list, unsigned int *num_attribs),
       (dpy, config, attrib_list, num_attribs))
STATUS(vaCreateSurfaces,
       (VADisplay dpy, unsigned int format, unsigned int width, unsigned int height, VASurfaceID *surfaces,
        unsigned int num_surfaces, VASurfaceAttrib *attrib_list, unsigned int num_attribs),
       (dpy, format, width, height, surfaces, num_surfaces, attrib_list, num_attribs))
STATUS(vaDestroySurfaces, (VADisplay dpy, VASurfaceID *surfaces, int num_surfaces), (dpy, surfaces, num_surfaces))
STATUS(vaSyncSurface, (VADisplay dpy, VASurfaceID render_target), (dpy, render_target))
STATUS(vaExportSurfaceHandle,
       (VADisplay dpy, VASurfaceID surface_id, uint32_t mem_type, uint32_t flags, void *descriptor),
       (dpy, surface_id, mem_type, flags, descriptor))
STATUS(vaCreateContext,
       (VADisplay dpy, VAConfigID config_id, int picture_width, int picture_height, int flag,
        VASurfaceID *render_targets, int num_render_targets, VAContextID *context),
       (dpy, config_id, picture_width, picture_height, flag, render_targets, num_render_targets, context))
STATUS(vaDestroyContext, (VADisplay dpy, VAContextID context), (dpy, context))
STATUS(vaCreateBuffer,
       (VADisplay dpy, VAContextID context, VABufferType type, unsigned int size, unsigned int num_elements,
        void *data, VABufferID *buf_id),
       (dpy, context, type, size, num_elements, data, buf_id))
STATUS(vaDestroyBuffer, (VADisplay dpy, VABufferID buffer_id), (dpy, buffer_id))
STATUS(vaMapBuffer, (VADisplay dpy, VABufferID buf_id, void **pbuf), (dpy, buf_id, pbuf))
VA(VAStatus, vaMapBuffer(dpy, buf_id, pbuf), vaMapBuffer2, (VADisplay dpy, VABufferID buf_id, void **pbuf, uint32_t flags),
   (dpy, buf_id, pbuf, flags))
STATUS(vaUnmapBuffer, (VADisplay dpy, VABufferID buf_id), (dpy, buf_id))
STATUS(vaSyncBuffer, (VADisplay dpy, VABufferID buf_id, uint64_t timeout_ns), (dpy, buf_id, timeout_ns))
STATUS(vaAcquireBufferHandle, (VADisplay dpy, VABufferID buf_id, VABufferInfo *buf_info), (dpy, buf_id, buf_info))
STATUS(vaReleaseBufferHandle, (VADisplay dpy, VABufferID buf_id), (dpy, buf_id))
STATUS(vaBeginPicture, (VADisplay dpy, VAContextID context, VASurfaceID render_target), (dpy, context, render_target))
STATUS(vaRenderPicture, (VADisplay dpy, VAContextID context, VABufferID *buffers, int num_buffers),
       (dpy, context, buffers, num_buffers))
STATUS(vaEndPicture, (VADisplay dpy, VAContextID context), (dpy, context))
STATUS(vaQueryImageFormats, (VADisplay dpy, VAImageFormat *format_list, int *num_formats),
       (dpy, format_list, num_formats))
STATUS(vaCreateImage, (VADisplay dpy, VAImageFormat *format, int width, int height, VAImage *image),
       (dpy, format, width, height, image))
STATUS(vaDestroyImage, (VADisplay dpy, VAImageID image), (dpy, image))
STATUS(vaDeriveImage, (VADisplay dpy, VASurfaceID surface, VAImage *image), (dpy, surface, image))
STATUS(vaGetImage,
       (VADisplay dpy, VASurfaceID surface, int x, int y, unsigned int width, unsigned int height, VAImageID image),
       (dpy, surface, x, y, width, height, image))
STATUS(vaPutImage,
       (VADisplay dpy, VASurfaceID surface, VAImageID image, int src_x, int src_y, unsigned int src_width,
        unsigned int src_height, int dest_x, int dest_y, unsigned int dest_width, unsigned int dest_height),
       (dpy, surface, image, src_x, src_y, src_width, src_height, dest_x, dest_y, dest_width, dest_height))
STATUS(vaQueryVideoProcFilters,
       (VADisplay dpy, VAContextID context, VAProcFilterType *filters, unsigned int *num_filters),
       (dpy, context, filters, num_filters))
STATUS(vaQueryVideoProcFilterCaps,
       (VADisplay dpy, VAContextID context, VAProcFilterType type, void *filter_caps, unsigned int *num_filter_caps),
       (dpy, context, type, filter_caps, num_filter_caps))
STATUS(vaQueryVideoProcPipelineCaps,
       (VADisplay dpy, VAContextID context, VABufferID *filters, unsigned int num_filters,
        VAProcPipelineCaps *pipeline_caps),
       (dpy, context, filters, num_filters, pipeline_caps))
