#pragma once
#include <stddef.h>
#include <stdint.h>

typedef struct ZTSession ZTSession;
typedef struct ZTTensor ZTTensor;
// Errors are owned strings; NULL means success. Tensors and sessions have explicit ownership.
char *zt_session(const char *path, ZTSession **result);
void zt_release_session(ZTSession *session);
char *zt_ints(const int32_t *values, const int64_t *shape, size_t rank, ZTTensor **result);
char *zt_run(ZTSession *session, const char *const *names, ZTTensor *const *inputs, size_t count,
             const char *const *outputs, size_t output_count, ZTTensor **result);
char *zt_doubles(ZTTensor *tensor, double *values, size_t count);
size_t zt_count(ZTTensor *tensor);
void zt_release_tensor(ZTTensor *tensor);
void zt_free_error(char *error);
