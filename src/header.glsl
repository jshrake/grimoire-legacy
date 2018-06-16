/*
GRIMOIRE BEGIN: default_shader_header.glsl
*/
#ifdef GL_ES
precision mediump float;
#endif

layout (std140) uniform GRIM_STATE {
  vec4  iMouse;
  vec4  iDate;
  vec3  iWindowResolution;
  float iTime;
  float iTimeDelta;
  float iFrame;
  float iFrameRate;
};
uniform vec3 iResolution;
uniform int iVertexCount;
/*
GRIMOIRE END: default_shader_header.glsl
*/
