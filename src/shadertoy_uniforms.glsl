/*
BEGIN: grim.glsl
*/
layout(std140) uniform GRIM_STATE {
  vec4 iMouse;
  vec4 iDate;
  vec3 iWindowResolution;
  float iTime;
  float iTimeDelta;
  float iFrame;
  float iFrameRate;
};
uniform vec3 iResolution;
uniform int iVertexCount;

#define GRIMOIRE
/*
END: grim.glsl
*/
