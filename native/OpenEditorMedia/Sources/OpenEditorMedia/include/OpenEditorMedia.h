#ifndef OPEN_EDITOR_MEDIA_H
#define OPEN_EDITOR_MEDIA_H

#include <stdbool.h>

#ifdef __cplusplus
extern "C" {
#endif

void *oe_player_create(void);
void oe_player_release(void *handle);
bool oe_player_attach(void *handle, void *ns_view, double x, double y, double width, double height);
bool oe_player_set_frame(void *handle, double x, double y, double width, double height);
bool oe_player_load_file(void *handle, const char *path);
void oe_player_play(void *handle);
void oe_player_pause(void *handle);
void oe_player_seek(void *handle, long long value, int timescale);

#ifdef __cplusplus
}
#endif

#endif
