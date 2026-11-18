cmake_minimum_required(VERSION 3.24)
# One version for the app and the addon: the desktop workspace's.
file(STRINGS "${CMAKE_CURRENT_SOURCE_DIR}/../desktop/Cargo.toml" zt_version_line REGEX "^version = \"[0-9.]+\"$")
string(REGEX REPLACE "^version = \"([0-9.]+)\"$" "\\1" zt_version "${zt_version_line}")
project(ZhuyinTypeFcitx5 VERSION ${zt_version} LANGUAGES CXX)

set(CMAKE_CXX_STANDARD 20)
set(CMAKE_CXX_STANDARD_REQUIRED ON)
set(CMAKE_POSITION_INDEPENDENT_CODE ON)
include(GNUInstallDirs)
find_package(Fcitx5Core 5.1.9 REQUIRED)
find_package(nlohmann_json 3.11 REQUIRED)

# The host the addon starts when nothing serves the broker endpoint (the packages install it in /usr/bin).
set(ZT_HOST_EXECUTABLE "${CMAKE_INSTALL_FULL_BINDIR}/zhuyintype" CACHE FILEPATH "Installed ZhuyinType host")

add_library(zhuyintype-fcitx-core STATIC src/Keys.cpp src/Broker.cpp src/ZhuyinType.cpp)
target_include_directories(zhuyintype-fcitx-core PUBLIC src)
target_link_libraries(zhuyintype-fcitx-core PUBLIC Fcitx5::Core nlohmann_json::nlohmann_json)
target_compile_definitions(zhuyintype-fcitx-core PRIVATE ZT_HOST_EXECUTABLE="${ZT_HOST_EXECUTABLE}")
target_compile_options(zhuyintype-fcitx-core PRIVATE -Wall -Wextra -Wpedantic -Werror)

add_library(zhuyintype MODULE src/Factory.cpp)
target_link_libraries(zhuyintype PRIVATE zhuyintype-fcitx-core)
configure_file(data/zhuyintype-addon.conf.in zhuyintype-addon.conf @ONLY)

# The addon directory comes from the Fcitx SDK (Debian/Ubuntu: /usr/lib/x86_64-linux-gnu/fcitx5, Fedora/openSUSE:
# /usr/lib64/fcitx5).  The packages (platforms/desktop/packaging/linux) place the same files.
install(TARGETS zhuyintype LIBRARY DESTINATION "${FCITX_INSTALL_ADDONDIR}")
install(FILES "${CMAKE_CURRENT_BINARY_DIR}/zhuyintype-addon.conf" DESTINATION "${FCITX_INSTALL_PKGDATADIR}/addon"
    RENAME zhuyintype.conf)
install(FILES data/zhuyintype.conf DESTINATION "${FCITX_INSTALL_PKGDATADIR}/inputmethod")
install(FILES LICENSES/nlohmann-json.txt DESTINATION "${CMAKE_INSTALL_DATADIR}/licenses/zhuyintype")
install(FILES ../../LICENSE DESTINATION "${CMAKE_INSTALL_DATADIR}/licenses/zhuyintype" RENAME AGPL-3.0.txt)

include(CTest)
if(BUILD_TESTING)
    # The installed framework plugins belong to the SDK, independent of our install prefix.
    file(GENERATE OUTPUT "${CMAKE_CURRENT_BINARY_DIR}/fcitx-test-paths.sh"
        CONTENT "system_addon_dir='$<TARGET_FILE_DIR:Fcitx5::Core>/fcitx5'\n")
    add_executable(fcitx-native-tests tests/NativeTest.cpp)
    target_link_libraries(fcitx-native-tests PRIVATE zhuyintype-fcitx-core)
    target_compile_options(fcitx-native-tests PRIVATE -Wall -Wextra -Wpedantic -Werror)
    find_package(Threads REQUIRED)
    add_executable(fcitx-broker-tests tests/BrokerTest.cpp)
    target_link_libraries(fcitx-broker-tests PRIVATE zhuyintype-fcitx-core Threads::Threads)
    target_compile_options(fcitx-broker-tests PRIVATE -Wall -Wextra -Wpedantic -Werror)
    add_test(NAME fcitx-broker-transport COMMAND fcitx-broker-tests)
    set_tests_properties(fcitx-broker-transport PROPERTIES TIMEOUT 10)

    # The tests below talk to a real host.  They are skipped (exit 77) while the host does not serve the broker yet;
    # ZT_REQUIRE_HOST=1 turns that into a failure.
    if(DEFINED ENV{CARGO_TARGET_DIR})
        set(zt_default_host "$ENV{CARGO_TARGET_DIR}/release/zhuyintype")
    else()
        set(zt_default_host "${CMAKE_CURRENT_SOURCE_DIR}/../desktop/target/release/zhuyintype")
    endif()
    set(ZT_IME_HOST "${zt_default_host}" CACHE FILEPATH "ZhuyinType host executable for the integration tests")
    add_test(NAME fcitx-native COMMAND bash "${CMAKE_CURRENT_SOURCE_DIR}/tests/with-host.sh"
        "${ZT_IME_HOST}" "$<TARGET_FILE:fcitx-native-tests>")
    set_tests_properties(fcitx-native PROPERTIES TIMEOUT 60 SKIP_RETURN_CODE 77)
    # 聯想詞 on: a host of its own, since the list after every commit would change the checks above.
    add_test(NAME fcitx-associated COMMAND env ZT_HOST_ASSOCIATED=1 bash "${CMAKE_CURRENT_SOURCE_DIR}/tests/with-host.sh"
        "${ZT_IME_HOST}" "$<TARGET_FILE:fcitx-native-tests>" associated)
    set_tests_properties(fcitx-associated PROPERTIES TIMEOUT 60 SKIP_RETURN_CODE 77)
    add_test(NAME fcitx-picker COMMAND bash "${CMAKE_CURRENT_SOURCE_DIR}/tests/with-host.sh"
        "${ZT_IME_HOST}" "$<TARGET_FILE:fcitx-native-tests>" picker)
    set_tests_properties(fcitx-picker PROPERTIES TIMEOUT 60 SKIP_RETURN_CODE 77)

    # Optional real application probe (a Qt Widgets window); the addon itself has no Qt dependency.
    find_package(Qt6 6.8 QUIET COMPONENTS Widgets)
    if(Qt6Widgets_FOUND)
        add_executable(fcitx-delivery-probe tests/DeliveryProbe.cpp)
        target_link_libraries(fcitx-delivery-probe PRIVATE Qt6::Widgets)
        add_executable(fcitx-self-host-probe tests/SelfHostProbe.cpp)
        target_link_libraries(fcitx-self-host-probe PRIVATE Qt6::Widgets)
        target_compile_options(fcitx-delivery-probe PRIVATE -Wall -Wextra -Wpedantic -Werror)
        target_compile_options(fcitx-self-host-probe PRIVATE -Wall -Wextra -Wpedantic -Werror)
        find_program(ZT_XVFB_RUN xvfb-run)
        find_program(ZT_XDOTOOL xdotool)
        find_program(ZT_XWININFO xwininfo)
        find_program(ZT_DBUS_RUN_SESSION dbus-run-session)
        if(ZT_XVFB_RUN AND ZT_XDOTOOL AND ZT_XWININFO AND ZT_DBUS_RUN_SESSION)
            add_test(NAME fcitx-application-delivery COMMAND bash
                "${CMAKE_CURRENT_SOURCE_DIR}/tests/delivery.sh" "${CMAKE_CURRENT_BINARY_DIR}" "${ZT_IME_HOST}")
            set_tests_properties(fcitx-application-delivery PROPERTIES TIMEOUT 60 SKIP_RETURN_CODE 77)
            add_test(NAME fcitx-self-host-settings COMMAND bash
                "${CMAKE_CURRENT_SOURCE_DIR}/tests/delivery.sh" "${CMAKE_CURRENT_BINARY_DIR}" "${ZT_IME_HOST}")
            set_tests_properties(fcitx-self-host-settings PROPERTIES TIMEOUT 60 SKIP_RETURN_CODE 77
                ENVIRONMENT "ZT_DELIVERY_SELF_HOST=1")
            # A HiDPI X11 desktop (Xft.dpi 192): Fcitx's caret is in X pixels, the app's window in its own.
            add_test(NAME fcitx-application-delivery-hidpi COMMAND bash
                "${CMAKE_CURRENT_SOURCE_DIR}/tests/delivery.sh" "${CMAKE_CURRENT_BINARY_DIR}" "${ZT_IME_HOST}")
            set_tests_properties(fcitx-application-delivery-hidpi PROPERTIES TIMEOUT 60 SKIP_RETURN_CODE 77
                ENVIRONMENT "GPUI_X11_SCALE_FACTOR=2")
            add_executable(fcitx-focus-probe tests/FocusProbe.cpp)
            target_link_libraries(fcitx-focus-probe PRIVATE Qt6::Widgets)
            target_compile_options(fcitx-focus-probe PRIVATE -Wall -Wextra -Wpedantic -Werror)
            add_test(NAME fcitx-focus-change COMMAND bash
                "${CMAKE_CURRENT_SOURCE_DIR}/tests/delivery.sh" "${CMAKE_CURRENT_BINARY_DIR}" "${ZT_IME_HOST}")
            set_tests_properties(fcitx-focus-change PROPERTIES TIMEOUT 60 SKIP_RETURN_CODE 77
                ENVIRONMENT "ZT_DELIVERY_FOCUS=1")
        endif()
    endif()
    # The same in a GTK 3 application (Fcitx's GTK module).
    find_package(PkgConfig QUIET)
    if(PkgConfig_FOUND)
        pkg_check_modules(ZT_GTK3 QUIET IMPORTED_TARGET gtk+-3.0)
    endif()
    if(ZT_GTK3_FOUND AND ZT_XVFB_RUN AND ZT_XDOTOOL AND ZT_DBUS_RUN_SESSION)
        add_executable(fcitx-gtk-focus-probe tests/GtkFocusProbe.cpp)
        target_link_libraries(fcitx-gtk-focus-probe PRIVATE PkgConfig::ZT_GTK3)
        target_compile_options(fcitx-gtk-focus-probe PRIVATE -Wall -Wextra -Werror)
        add_test(NAME fcitx-gtk-focus-change COMMAND bash
            "${CMAKE_CURRENT_SOURCE_DIR}/tests/delivery.sh" "${CMAKE_CURRENT_BINARY_DIR}" "${ZT_IME_HOST}")
        set_tests_properties(fcitx-gtk-focus-change PROPERTIES TIMEOUT 60 SKIP_RETURN_CODE 77 ENVIRONMENT "ZT_DELIVERY_GTK=1")
    endif()
endif()
