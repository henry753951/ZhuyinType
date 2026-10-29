import QtQuick
import QtQuick.Shapes
import "Icons.js" as Icons

// A line icon from Icons.js, stroked in one colour at any size.
Item {
    id: icon
    property string name: ""
    property color color: "black"
    property real size: 16
    property real weight: 1.8
    implicitWidth: size
    implicitHeight: size
    Accessible.ignored: true
    Shape {
        visible: icon.name.length > 0
        width: 24
        height: 24
        scale: icon.width / 24
        transformOrigin: Item.TopLeft
        preferredRendererType: Shape.CurveRenderer
        ShapePath {
            strokeColor: icon.color
            // The shape is scaled from the 24-unit grid, so the stroke is set in grid units to land at `weight` pixels.
            strokeWidth: icon.weight * 24 / Math.max(1, icon.width)
            fillColor: "transparent"
            capStyle: ShapePath.RoundCap
            joinStyle: ShapePath.RoundJoin
            PathSvg {
                path: Icons.paths[icon.name] || ""
            }
        }
    }
}
