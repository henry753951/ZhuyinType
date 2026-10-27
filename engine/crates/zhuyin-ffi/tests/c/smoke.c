/* The C ABI from C: an engine from lexicon text, a composer with a reranker hook, the presentation, a commit and
 * personal data.  Built and run by tests/c_program.rs; exits 0 when every check passes. */
#include <stdio.h>
#include <stdlib.h>
#include <string.h>

#include "zhuyin.h"

static int failures = 0;

#define CHECK(condition, message)                                   \
    do {                                                            \
        if (!(condition)) {                                         \
            fprintf(stderr, "FAIL %s (line %d)\n", message, __LINE__); \
            failures++;                                             \
        }                                                           \
    } while (0)

static const char *lexicon =
    "ㄋㄧˇ\t你\t-3.0\nㄋㄧˇ\t妳\t-4.0\nㄏㄠˇ\t好\t-3.0\nㄏㄠˇ\t郝\t-6.0\nㄋㄧˇ ㄏㄠˇ\t你好\t-2.0\n";

static int choices = 0;
static int released = 0;

/* Keeps the engine's first sentence, counting the calls. */
static ptrdiff_t choose_first(void *context, const ZtSentences *sentences) {
    (void)context;
    choices++;
    if (zt_sentences_count(sentences) > 0) {
        char *text = zt_sentences_text(sentences, 0);
        if (text == NULL || text[0] == '\0') failures++;
        zt_string_free(text);
    }
    return 0;
}

static void release_hook(void *context) {
    (void)context;
    released++;
}

/* Types ASCII keys (space for Space, newline for Return); returns what was committed (caller frees). */
static char *type(ZtComposer *composer, const char *keys) {
    char committed[256] = "";
    for (const char *c = keys; *c; c++) {
        ZtKey key = {ZT_KEY_CHAR, (uint32_t)(unsigned char)*c};
        if (*c == ' ') key = (ZtKey){ZT_KEY_SPACE, 0};
        if (*c == '\n') key = (ZtKey){ZT_KEY_ENTER, 0};
        ZtKeyResult result = zt_composer_handle(composer, key);
        if (result.commit != NULL) {
            strncat(committed, result.commit, sizeof committed - strlen(committed) - 1);
            zt_string_free(result.commit);
        }
    }
    char *copy = malloc(strlen(committed) + 1);
    if (copy != NULL) memcpy(copy, committed, strlen(committed) + 1);
    return copy;
}

int main(void) {
    char *error = NULL;
    ZtEngine *engine = zt_engine_open_text(lexicon, NULL, NULL, NULL, &error);
    CHECK(engine != NULL, "engine opens");
    if (engine == NULL) {
        fprintf(stderr, "%s\n", error ? error : "?");
        return 1;
    }

    ZtComposerOptions options = zt_composer_options_default();
    options.learning_enabled = false;
    ZtComposer *composer = zt_composer_new(engine, &options);
    CHECK(composer != NULL, "composer");
    zt_composer_set_reranker(composer, (ZtRerankerHook){NULL, choose_first, release_hook});

    char *nothing = type(composer, "su3cl3");
    CHECK(strcmp(nothing, "") == 0, "nothing committed while composing");
    free(nothing);
    CHECK(zt_composer_is_composing(composer), "composing");
    CHECK(choices > 0, "the reranker hook was asked");

    ZtPresentation *p = zt_composer_presentation(composer);
    CHECK(strcmp(p->text, "你好") == 0, "presentation text");
    CHECK(p->cursor == 2 && p->clause_count == 1, "cursor and clause");
    CHECK(p->candidates == NULL && p->focused_clause == -1, "no window");
    zt_presentation_free(p);

    char *committed = type(composer, "\n");
    CHECK(strcmp(committed, "你好") == 0, "Return commits");
    free(committed);
    CHECK(!zt_composer_is_composing(composer), "composition done");

    /* The candidate window. */
    char *open = type(composer, "su3 ");
    free(open);
    p = zt_composer_presentation(composer);
    CHECK(p->candidates != NULL && p->candidates->count >= 2, "window open");
    if (p->candidates != NULL && p->candidates->count >= 2) {
        CHECK(strcmp(p->candidates->items[1].text, "妳") == 0, "second candidate");
    }
    zt_presentation_free(p);
    ZtKeyResult pick = zt_composer_select_candidate(composer, 1);
    zt_string_free(pick.commit);
    char *rest = zt_composer_commit_all(composer);
    CHECK(strcmp(rest, "妳") == 0, "picked candidate commits");
    zt_string_free(rest);

    /* Personal data. */
    CHECK(zt_engine_add_user_phrase(engine, "郝好", "ㄏㄠˇ ㄏㄠˇ", &error), "user phrase added");
    ZtPhraseList *phrases = zt_engine_user_phrases(engine);
    CHECK(phrases->count == 1 && strcmp(phrases->items[0].word, "郝好") == 0, "user phrase listed");
    zt_phrase_list_free(phrases);
    char *exported = zt_engine_export_personal_data(engine);
    CHECK(exported != NULL && strstr(exported, "郝好") != NULL, "export");
    zt_string_free(exported);

    zt_composer_free(composer);
    CHECK(released == 1, "hook released once");
    zt_engine_free(engine);
    if (failures == 0) printf("C ABI smoke test passed\n");
    return failures == 0 ? 0 : 1;
}
