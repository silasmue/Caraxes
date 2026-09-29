// SPDX-License-Identifier: MIT
// SPDX-FileCopyrightText: 2026 Silas Müller <github@silasmueller.de>
// A constant colour, so the check can compare exact bytes.
#version 450
layout(location = 0) out vec4 color;
void main() { color = vec4(1.0, 0.5, 0.0, 1.0); }
