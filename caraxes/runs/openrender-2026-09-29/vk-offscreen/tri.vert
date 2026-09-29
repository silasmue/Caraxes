// SPDX-License-Identifier: MIT
// SPDX-FileCopyrightText: 2026 Silas Müller <github@silasmueller.de>
// One triangle without vertex buffers: the positions come from gl_VertexIndex.
#version 450
const vec2 pos[3] = vec2[](vec2(0.0, -0.8), vec2(0.8, 0.8), vec2(-0.8, 0.8));
void main() { gl_Position = vec4(pos[gl_VertexIndex], 0.0, 1.0); }
