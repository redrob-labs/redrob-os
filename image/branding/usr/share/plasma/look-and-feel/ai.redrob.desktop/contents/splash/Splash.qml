/*
 * Redrob OS login splash (KSplash, Plasma look-and-feel ai.redrob.desktop).
 * Copyright 2026 Janghoon Lee (Redrob) and contributors
 * SPDX-License-Identifier: Apache-2.0
 *
 * Same composition as the boot splash: the primary-white lockup (design-system
 * export, never redrawn) still on Redrob Black, a loader under it. The lockup
 * does not animate (10-logo.md: the mark is never a loading indicator); the
 * bar fills with KSplash's stage (1..6). Colours are redrob-ui tokens:
 *   redrob-black #0A0B0C (field), gray-8 #292E37 (track),
 *   blue-6 #2B52FF -> accent-teal-3 #6FF4F0 (gradient-brand, the fill).
 */
import QtQuick 2.15

Rectangle {
    id: root
    color: "#0a0b0c"

    property int stage

    Image {
        id: lockup
        source: "images/lockup.png"
        anchors.horizontalCenter: parent.horizontalCenter
        anchors.verticalCenter: parent.verticalCenter
        anchors.verticalCenterOffset: -height / 2
        width: 420
        height: 84
        fillMode: Image.PreserveAspectFit
        smooth: true
    }

    Rectangle {
        id: track
        width: 300
        height: 4
        color: "#292e37"
        anchors.horizontalCenter: parent.horizontalCenter
        anchors.top: lockup.bottom
        anchors.topMargin: 42

        Rectangle {
            height: parent.height
            width: parent.width * Math.min(Math.max(root.stage, 0), 6) / 6
            gradient: Gradient {
                orientation: Gradient.Horizontal
                GradientStop { position: 0.0; color: "#2b52ff" }
                GradientStop { position: 1.0; color: "#6ff4f0" }
            }
            Behavior on width { NumberAnimation { duration: 140 } }
        }
    }
}
