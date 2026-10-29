cmake_minimum_required(VERSION 3.24)
project(ZhuyinTypeDesktop VERSION 0.1.0 LANGUAGES CXX)

set(CMAKE_CXX_STANDARD 20)
set(CMAKE_CXX_STANDARD_REQUIRED ON)
if(MSVC)
    add_compile_options(/utf-8)
endif()
find_package(Qt6 6.8 REQUIRED COMPONENTS Quick QuickControls2 Test QuickTest Network)
qt_standard_project_setup(REQUIRES 6.8)

add_library(desktop_state STATIC src/Settings.cpp src/Settings.h src/Candidates.cpp src/Candidates.h src/EngineSession.cpp src/EngineSession.h src/Shortcuts.cpp src/Shortcuts.h src/DesktopServices.cpp src/DesktopServices.h src/PlatformSettings.cpp src/PlatformSettings.h src/LegalDocuments.cpp src/LegalDocuments.h src/InputController.cpp src/InputController.h src/ImeHost.cpp src/ImeHost.h src/ImeEndpoint.cpp src/ImeEndpoint.h src/KeyInput.cpp src/KeyInput.h src/Presentation.h src/CorrectionLog.cpp src/CorrectionLog.h src/Practice.cpp src/Practice.h)
target_link_libraries(desktop_state PUBLIC Qt6::Core Qt6::Gui Qt6::Network)
if(WIN32)
    target_link_libraries(desktop_state PUBLIC advapi32 psapi)
endif()
target_include_directories(desktop_state PUBLIC src ../../engine/crates/zhuyin-ffi/include)
qt_add_resources(desktop_state settings PREFIX / FILES settings.json)

set(ZT_ENGINE_LIBRARY "${CMAKE_CURRENT_SOURCE_DIR}/../../engine/target/release/${CMAKE_SHARED_LIBRARY_PREFIX}zhuyin${CMAKE_SHARED_LIBRARY_SUFFIX}" CACHE FILEPATH "Rust C ABI library")
get_filename_component(ZT_ENGINE_LIBRARY_NAME "${ZT_ENGINE_LIBRARY}" NAME)
qt_add_executable(zhuyintype-desktop src/main.cpp src/FontSetup.cpp src/ApplicationInfo.cpp src/PreviewCapture.cpp src/PreviewCapture.h)
target_compile_definitions(zhuyintype-desktop PRIVATE ZT_ENGINE_LIBRARY_NAME="${ZT_ENGINE_LIBRARY_NAME}" ZT_DEVELOPMENT_DATA="${CMAKE_CURRENT_SOURCE_DIR}/../../data/build")
add_custom_command(TARGET zhuyintype-desktop POST_BUILD COMMAND ${CMAKE_COMMAND} -E copy_if_different "${ZT_ENGINE_LIBRARY}" "$<TARGET_FILE_DIR:zhuyintype-desktop>/${ZT_ENGINE_LIBRARY_NAME}")
file(STRINGS "../../engine/Cargo.toml" LICENSE_LINE REGEX "^license =")
string(REGEX REPLACE "license = \"([^\"]+)\"" "\\1" ZT_LICENSE "${LICENSE_LINE}")
file(READ "../../platforms/macos/Resources/Info.plist" APP_PLIST)
string(REGEX MATCH "CFBundleShortVersionString</key>[\r\n\t ]*<string>([^<]+)" VERSION_MATCH "${APP_PLIST}")
set(ZT_APP_VERSION "${CMAKE_MATCH_1}")
string(REGEX MATCH "CFBundleVersion</key>[\r\n\t ]*<string>([^<]+)" BUILD_MATCH "${APP_PLIST}")
set(ZT_APP_BUILD "${CMAKE_MATCH_1}")
configure_file(src/metadata.json.in metadata.json @ONLY)
qt_add_resources(zhuyintype-desktop metadata PREFIX /metadata BASE "${CMAKE_CURRENT_BINARY_DIR}" FILES "${CMAKE_CURRENT_BINARY_DIR}/metadata.json")
# 授權與資料來源: resources/sources.json and every licence text it names, read inside the app (LegalDocuments).
set(ZT_LICENSES "${CMAKE_CURRENT_BINARY_DIR}/legal/Licenses")
file(GLOB ZT_SHARED_LICENSES LIST_DIRECTORIES false "${CMAKE_CURRENT_SOURCE_DIR}/../macos/Resources/Licenses/*"
    "${CMAKE_CURRENT_SOURCE_DIR}/licenses/*" "${CMAKE_CURRENT_SOURCE_DIR}/../linux/LICENSES/*")
set(ZT_LICENSE_FILES "${ZT_LICENSES}/AGPL-3.0.txt" "${ZT_LICENSES}/NOTICE.txt" "${ZT_LICENSES}/sources.json" "${ZT_LICENSES}/Rust-crates.txt")
set(ZT_LICENSE_COPIES "")
foreach(license IN LISTS ZT_SHARED_LICENSES)
    get_filename_component(license_name "${license}" NAME)
    list(APPEND ZT_LICENSE_FILES "${ZT_LICENSES}/${license_name}")
    list(APPEND ZT_LICENSE_COPIES COMMAND ${CMAKE_COMMAND} -E copy_if_different "${license}" "${ZT_LICENSES}/${license_name}")
endforeach()
add_custom_command(OUTPUT ${ZT_LICENSE_FILES}
    COMMAND ${CMAKE_COMMAND} -E make_directory "${ZT_LICENSES}"
    COMMAND ${CMAKE_COMMAND} -E copy_if_different "${CMAKE_CURRENT_SOURCE_DIR}/../../LICENSE" "${ZT_LICENSES}/AGPL-3.0.txt"
    COMMAND ${CMAKE_COMMAND} -E copy_if_different "${CMAKE_CURRENT_SOURCE_DIR}/../../NOTICE" "${ZT_LICENSES}/NOTICE.txt"
    COMMAND ${CMAKE_COMMAND} -E copy_if_different "${CMAKE_CURRENT_SOURCE_DIR}/../../resources/sources.json" "${ZT_LICENSES}/sources.json"
    ${ZT_LICENSE_COPIES}
    COMMAND ${CMAKE_COMMAND} -DENGINE_DIR=${CMAKE_CURRENT_SOURCE_DIR}/../../engine -DOUTPUT=${ZT_LICENSES}/Rust-crates.txt
        -P ${CMAKE_CURRENT_SOURCE_DIR}/cmake/RustLicenses.cmake
    DEPENDS ../../LICENSE ../../NOTICE ../../resources/sources.json ../../engine/Cargo.lock cmake/RustLicenses.cmake ${ZT_SHARED_LICENSES}
    COMMENT "Collecting licence texts")
qt_add_resources(zhuyintype-desktop legal PREFIX /legal BASE "${CMAKE_CURRENT_BINARY_DIR}/legal" FILES ${ZT_LICENSE_FILES})
if(APPLE)
    set_target_properties(zhuyintype-desktop PROPERTIES
        MACOSX_BUNDLE TRUE
        MACOSX_BUNDLE_GUI_IDENTIFIER org.zhuyintype.desktop.preview
        MACOSX_BUNDLE_BUNDLE_NAME "ZhuyinType Desktop Preview"
        MACOSX_BUNDLE_ICON_FILE AppIcon.icns)
    set_source_files_properties(../macos/Resources/AppIcon.icns PROPERTIES MACOSX_PACKAGE_LOCATION Resources)
    target_sources(zhuyintype-desktop PRIVATE ../macos/Resources/AppIcon.icns)
endif()
if(WIN32)
    configure_file(src/windows.rc.in windows.rc @ONLY)
    target_sources(zhuyintype-desktop PRIVATE "${CMAKE_CURRENT_BINARY_DIR}/windows.rc")
endif()
qt_add_resources(zhuyintype-desktop branding PREFIX /branding
    BASE ../macos/Resources FILES ../macos/Resources/AppIcon.icns)
qt_add_qml_module(zhuyintype-desktop URI ZhuyinType.Desktop VERSION 1.0
    QML_FILES qml/Main.qml qml/Theme.qml qml/CandidatePanel.qml qml/SettingRow.qml qml/SettingsPage.qml
    qml/Pages.js qml/SectionCard.qml qml/NavigationRow.qml qml/AppearancePage.qml
    qml/InputPages.qml qml/OverviewPages.qml qml/AboutPages.qml qml/LibraryPages.qml qml/ShortcutOverview.qml qml/ShortcutRecorder.qml qml/LiveInput.qml qml/ImePopup.qml
    qml/Icons.js qml/Icon.qml qml/AppButton.qml qml/AppTextField.qml qml/AppComboBox.qml qml/AppDialog.qml qml/AppDialogButtons.qml qml/AppMenu.qml
    qml/KeyCap.qml qml/Chip.qml qml/EmptyState.qml qml/IconButton.qml qml/Divider.qml qml/Caption.qml qml/DemoControls.qml
    qml/SegmentedControl.qml qml/StatTile.qml qml/ValueRow.qml qml/DocumentText.qml qml/MarkdownText.qml qml/FeatureCard.qml
    qml/ListRow.qml qml/KeyCapFlow.qml qml/DemoLine.qml qml/MiniDemo.qml qml/MiniList.qml qml/ChoiceDemo.qml qml/InfoPopover.qml
    qml/FactRow.qml qml/UsageMeter.qml qml/DisclosureRow.qml qml/DocumentViewer.qml
    qml/SettingsRow.qml qml/SidebarItem.qml qml/PillTabs.qml qml/SelectableRow.qml qml/AppTextArea.qml
    qml/IntroStage.qml qml/BarChart.qml qml/MeterBar.qml qml/WeekHeatMap.qml qml/OptionTile.qml qml/WizardWindow.qml)
target_link_libraries(zhuyintype-desktop PRIVATE desktop_state Qt6::Quick Qt6::QuickControls2)
if(MSVC)
    # Qt's generated QML cache instantiates unreachable template paths in Release.
    # Keep /W4 /WX for our sources; suppress C4702 only in generated cache code.
    get_target_property(desktop_sources zhuyintype-desktop SOURCES)
    foreach(source IN LISTS desktop_sources)
        if(source MATCHES "/qmlcache/.*\\.cpp$")
            set_property(SOURCE "${source}" APPEND PROPERTY COMPILE_OPTIONS /wd4702)
        endif()
    endforeach()
endif()

include(CTest)
if(BUILD_TESTING)
    qt_add_executable(desktop-interactions tests/QuickTest.cpp)
    qt_add_resources(desktop-interactions test_branding PREFIX /branding
        BASE ../macos/Resources FILES ../macos/Resources/AppIcon.icns)
    target_link_libraries(desktop-interactions PRIVATE Qt6::QuickTest Qt6::QuickControls2)
    target_compile_definitions(desktop-interactions PRIVATE QUICK_TEST_SOURCE_DIR="${CMAKE_CURRENT_SOURCE_DIR}/tests")
    add_test(NAME desktop-interactions COMMAND desktop-interactions)
    set_tests_properties(desktop-interactions PROPERTIES ENVIRONMENT "QT_QPA_PLATFORM=offscreen;QT_QUICK_BACKEND=software" TIMEOUT 60)
    foreach(suite Shortcut DesktopServices EngineSession PlatformSettings LegalDocuments InputController ImeHost)
        qt_add_executable(test-${suite} tests/${suite}Test.cpp)
        target_link_libraries(test-${suite} PRIVATE desktop_state Qt6::Test)
        add_test(NAME ${suite} COMMAND test-${suite})
        set_tests_properties(${suite} PROPERTIES ENVIRONMENT "ZHUYINTYPE_TEST_LIBRARY=${ZT_ENGINE_LIBRARY};ZT_ENGINE_LIBRARY=${ZT_ENGINE_LIBRARY};QT_QPA_PLATFORM=offscreen" TIMEOUT 60)
    endforeach()
    qt_add_executable(desktop-tests tests/StateTest.cpp)
    target_link_libraries(desktop-tests PRIVATE desktop_state Qt6::Test)
    add_test(NAME desktop-state COMMAND desktop-tests)
    add_test(NAME design-rules COMMAND ${CMAKE_COMMAND} -DQML_DIR=${CMAKE_CURRENT_SOURCE_DIR}/qml
        -DCMAKE_LISTS=${CMAKE_CURRENT_SOURCE_DIR}/CMakeLists.txt -P ${CMAKE_CURRENT_SOURCE_DIR}/tests/DesignLint.cmake)
    add_test(NAME desktop-render COMMAND zhuyintype-desktop --render-dir ${CMAKE_CURRENT_BINARY_DIR}/renders)
    set_tests_properties(desktop-render PROPERTIES ENVIRONMENT "QT_QPA_PLATFORM=offscreen;QT_QUICK_BACKEND=software" TIMEOUT 60)
    if(WIN32)
        # windeployqt intentionally deploys only qwindows; headless tests need the SDK's offscreen plugin.
        get_target_property(qt_qmake Qt6::qmake IMPORTED_LOCATION)
        execute_process(COMMAND "${qt_qmake}" -query QT_INSTALL_PLUGINS
            OUTPUT_VARIABLE qt_test_plugins OUTPUT_STRIP_TRAILING_WHITESPACE COMMAND_ERROR_IS_FATAL ANY)
        set_property(TEST desktop-interactions Shortcut DesktopServices EngineSession PlatformSettings LegalDocuments
            InputController ImeHost desktop-state desktop-render APPEND PROPERTY ENVIRONMENT
            "QT_PLUGIN_PATH=${qt_test_plugins}" "QT_QPA_FONTDIR=$ENV{WINDIR}/Fonts")
    endif()
endif()

foreach(target desktop_state zhuyintype-desktop)
    if(MSVC)
        target_compile_options(${target} PRIVATE /W4 /WX /utf-8)
    else()
        target_compile_options(${target} PRIVATE -Wall -Wextra -Wpedantic -Werror)
    endif()
endforeach()
