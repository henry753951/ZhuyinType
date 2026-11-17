// The focus probe (FocusProbe.cpp) in a GTK 3 application with Fcitx's GTK module (delivery.sh, ZT_DELIVERY_GTK=1).
// GTK keeps an input context per field, and commits the composition itself when a field loses focus.
#include <gtk/gtk.h>
#include <gdk/gdkx.h>
#include <glib.h>
#include <cstdio>
#include <string>

namespace {
struct Field {
    GtkWidget *entry = nullptr;
    std::string preedit;
};
struct Probe {
    Field first, second, other;
    GtkWidget *window = nullptr, *otherWindow = nullptr;
    int phase = 0, settle = 0, status = 1;
};

bool run(const std::string &command) {
    gint code = 0;
    return g_spawn_command_line_sync(command.c_str(), nullptr, nullptr, &code, nullptr) && g_spawn_check_wait_status(code, nullptr);
}
std::string text(const Field &field) { return gtk_entry_get_text(GTK_ENTRY(field.entry)); }
std::string xid(GtkWidget *window) { return std::to_string(GDK_WINDOW_XID(gtk_widget_get_window(window))); }

void watch(Field &field) {
    g_signal_connect(field.entry, "preedit-changed", G_CALLBACK(+[](GtkEntry *, gchar *preedit, gpointer data) {
        static_cast<Field *>(data)->preedit = preedit;
    }), &field);
}

void finish(Probe &probe, int status, const char *what) {
    if (status != 0)
        std::fprintf(stderr, "FAIL %s at phase %d: first=\"%s\" second=\"%s\" other=\"%s\"\n", what, probe.phase,
            text(probe.first).c_str(), text(probe.second).c_str(), text(probe.other).c_str());
    probe.status = status;
    gtk_main_quit();
}

gboolean tick(gpointer data) {
    auto &probe = *static_cast<Probe *>(data);
    const auto type = [](const char *keys) {
        return run(std::string("xdotool type --clearmodifiers --delay 50 ") + keys);
    };
    const auto composed = [](const Field &field, const char *expected) { return field.preedit == expected; };
    switch (probe.phase) {
    case 0:
        if (!run("xdotool windowfocus --sync " + xid(probe.window)) || !run("fcitx5-remote -s zhuyintype") ||
            !run("fcitx5-remote -o") || !type("su3cl3")) { finish(probe, 2, "xdotool or fcitx5-remote"); return G_SOURCE_REMOVE; }
        ++probe.phase; probe.settle = 0;
        break;
    case 1:
        if (!composed(probe.first, "你好")) break;
        gtk_widget_grab_focus(probe.second.entry);
        ++probe.phase; probe.settle = 0;
        break;
    case 2:
        if (++probe.settle < 10) break;
        if (text(probe.first) != "你好" || !text(probe.second).empty()) { finish(probe, 3, "moving to another field"); return G_SOURCE_REMOVE; }
        // Each GTK field has an input context of its own, which starts in the group's first input method.
        if (!run("fcitx5-remote -o") || !type("su3")) { finish(probe, 2, "xdotool or fcitx5-remote"); return G_SOURCE_REMOVE; }
        ++probe.phase; probe.settle = 0;
        break;
    case 3:
        if (!composed(probe.second, "你")) break;
        if (!run("xdotool windowfocus --sync " + xid(probe.otherWindow))) { finish(probe, 2, "xdotool"); return G_SOURCE_REMOVE; }
        ++probe.phase; probe.settle = 0;
        break;
    case 4:
        if (++probe.settle < 10) break;
        if (text(probe.second) != "你" || !text(probe.other).empty() || text(probe.first) != "你好") {
            finish(probe, 3, "moving to another window"); return G_SOURCE_REMOVE;
        }
        if (!run("fcitx5-remote -o") || !type("cl3")) { finish(probe, 2, "xdotool or fcitx5-remote"); return G_SOURCE_REMOVE; }
        ++probe.phase; probe.settle = 0;
        break;
    case 5:
        if (!composed(probe.other, "好")) break;
        if (!run("xdotool key --clearmodifiers Return")) { finish(probe, 2, "xdotool"); return G_SOURCE_REMOVE; }
        ++probe.phase;
        break;
    case 6:
        if (text(probe.other) != "好" || !probe.other.preedit.empty()) break;
        std::printf("PASS GTK 3: the composition reaches its own field once, in the same window and across windows\n");
        finish(probe, 0, "");
        return G_SOURCE_REMOVE;
    }
    return G_SOURCE_CONTINUE;
}
}

int main(int argc, char **argv) {
    gtk_init(&argc, &argv);
    Probe probe;
    probe.window = gtk_window_new(GTK_WINDOW_TOPLEVEL);
    gtk_window_set_title(GTK_WINDOW(probe.window), "ZhuyinType GTK focus probe");
    auto *box = gtk_box_new(GTK_ORIENTATION_VERTICAL, 4);
    probe.first.entry = gtk_entry_new();
    probe.second.entry = gtk_entry_new();
    gtk_container_add(GTK_CONTAINER(box), probe.first.entry);
    gtk_container_add(GTK_CONTAINER(box), probe.second.entry);
    gtk_container_add(GTK_CONTAINER(probe.window), box);
    gtk_window_set_default_size(GTK_WINDOW(probe.window), 500, 100);
    gtk_window_move(GTK_WINDOW(probe.window), 200, 160);
    probe.otherWindow = gtk_window_new(GTK_WINDOW_TOPLEVEL);
    gtk_window_set_title(GTK_WINDOW(probe.otherWindow), "ZhuyinType GTK focus probe: other window");
    probe.other.entry = gtk_entry_new();
    gtk_container_add(GTK_CONTAINER(probe.otherWindow), probe.other.entry);
    gtk_window_set_default_size(GTK_WINDOW(probe.otherWindow), 500, 50);
    gtk_window_move(GTK_WINDOW(probe.otherWindow), 200, 400);
    for (auto *field : {&probe.first, &probe.second, &probe.other}) watch(*field);
    gtk_widget_show_all(probe.window);
    gtk_widget_show_all(probe.otherWindow);
    gtk_window_present(GTK_WINDOW(probe.window));
    gtk_widget_grab_focus(probe.first.entry);
    g_timeout_add(500, +[](gpointer data) {
        g_timeout_add(100, tick, data);
        return G_SOURCE_REMOVE;
    }, &probe);
    g_timeout_add(20000, +[](gpointer data) {
        finish(*static_cast<Probe *>(data), 1, "timed out");
        return G_SOURCE_REMOVE;
    }, &probe);
    gtk_main();
    return probe.status;
}
