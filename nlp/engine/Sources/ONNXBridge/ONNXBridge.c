#include "ONNXBridge.h"
#include <onnxruntime_c_api.h>
#include <stdlib.h>
#include <string.h>

struct ZTSession { OrtEnv *env; OrtSession *session; };
struct ZTTensor { OrtValue *value; };
static const OrtApi *api(void) { return OrtGetApiBase()->GetApi(ORT_API_VERSION); }
static char *error(OrtStatus *status) {
    if (!status) return NULL;
    char *message = strdup(api()->GetErrorMessage(status));
    api()->ReleaseStatus(status);
    return message;
}
#define CHECK(call) do { char *e = error(call); if (e) { failure = e; goto cleanup; } } while (0)

char *zt_session(const char *path, ZTSession **result) {
    char *failure = NULL;
    OrtSessionOptions *options = NULL;
    ZTSession *s = calloc(1, sizeof(*s));
    if (!s) return strdup("Cannot allocate ONNX session");
    CHECK(api()->CreateEnv(ORT_LOGGING_LEVEL_WARNING, "ZhuyinType", &s->env));
    CHECK(api()->DisableTelemetryEvents(s->env));
    CHECK(api()->CreateSessionOptions(&options));
    // The evaluator supplies process parallelism; ORT must not spawn 64 threads per worker.
    CHECK(api()->SetIntraOpNumThreads(options, 1));
    CHECK(api()->SetInterOpNumThreads(options, 1));
    CHECK(api()->SetSessionGraphOptimizationLevel(options, ORT_ENABLE_ALL));
    CHECK(api()->CreateSession(s->env, path, options, &s->session));
cleanup:
    if (options) api()->ReleaseSessionOptions(options);
    if (failure) zt_release_session(s); else *result = s;
    return failure;
}
void zt_release_session(ZTSession *s) {
    if (!s) return;
    if (s->session) api()->ReleaseSession(s->session);
    if (s->env) api()->ReleaseEnv(s->env);
    free(s);
}
char *zt_ints(const int32_t *values, const int64_t *shape, size_t rank, ZTTensor **result) {
    char *failure = NULL;
    OrtAllocator *allocator = NULL;
    ZTTensor *t = calloc(1, sizeof(*t));
    if (!t) return strdup("Cannot allocate ONNX tensor");
    CHECK(api()->GetAllocatorWithDefaultOptions(&allocator));
    CHECK(api()->CreateTensorAsOrtValue(allocator, shape, rank, ONNX_TENSOR_ELEMENT_DATA_TYPE_INT32, &t->value));
    void *data = NULL;
    CHECK(api()->GetTensorMutableData(t->value, &data));
    memcpy(data, values, zt_count(t) * sizeof(int32_t));
cleanup:
    if (failure) zt_release_tensor(t); else *result = t;
    return failure;
}
char *zt_run(ZTSession *s, const char *const *names, ZTTensor *const *inputs, size_t count,
             const char *const *outputs, size_t output_count, ZTTensor **result) {
    char *failure = NULL;
    const OrtValue **in = calloc(count, sizeof(*in));
    OrtValue **out = calloc(output_count, sizeof(*out));
    if (!in || !out) { free(in); free(out); return strdup("Cannot allocate ONNX call"); }
    for (size_t i = 0; i < count; i++) in[i] = inputs[i]->value;
    CHECK(api()->Run(s->session, NULL, names, in, count, outputs, output_count, out));
    for (size_t i = 0; i < output_count; i++) {
        result[i] = calloc(1, sizeof(ZTTensor));
        if (!result[i]) {
            for (size_t j = 0; j < i; j++) { zt_release_tensor(result[j]); result[j] = NULL; }
            failure = strdup("Cannot allocate ONNX result"); goto cleanup;
        }
        result[i]->value = out[i]; out[i] = NULL;
    }
cleanup:
    for (size_t i = 0; i < output_count; i++) if (out[i]) api()->ReleaseValue(out[i]);
    free(in); free(out);
    return failure;
}
size_t zt_count(ZTTensor *t) {
    OrtTensorTypeAndShapeInfo *info = NULL;
    size_t count = 0;
    OrtStatus *status = api()->GetTensorTypeAndShape(t->value, &info);
    if (!status) status = api()->GetTensorShapeElementCount(info, &count);
    if (info) api()->ReleaseTensorTypeAndShapeInfo(info);
    if (status) api()->ReleaseStatus(status);
    return count;
}
char *zt_doubles(ZTTensor *t, double *values, size_t count) {
    char *failure = NULL;
    OrtTensorTypeAndShapeInfo *info = NULL;
    ONNXTensorElementDataType type;
    void *data = NULL;
    if (count != zt_count(t)) return strdup("ONNX output size mismatch");
    CHECK(api()->GetTensorTypeAndShape(t->value, &info));
    CHECK(api()->GetTensorElementType(info, &type));
    CHECK(api()->GetTensorMutableData(t->value, &data));
    if (type != ONNX_TENSOR_ELEMENT_DATA_TYPE_FLOAT) {
        failure = strdup("Expected fp32 ONNX logprob output"); goto cleanup;
    }
    for (size_t i = 0; i < count; i++) values[i] = ((float *)data)[i];
cleanup:
    if (info) api()->ReleaseTensorTypeAndShapeInfo(info);
    return failure;
}
void zt_release_tensor(ZTTensor *t) {
    if (!t) return;
    if (t->value) api()->ReleaseValue(t->value);
    free(t);
}
void zt_free_error(char *e) { free(e); }
