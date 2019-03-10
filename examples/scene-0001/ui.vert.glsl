#include "common.glsl"

out vec4 v_color;
void main() {
  float id = vertexId;
  float ar = iResolution.z;
  // Translate to the upper-left corner and shrink it down
  mat4 TS = transpose(vec3(-0.85, 0.85, 0.)) * mat4(scale(.1));
  vec3 vertex = arcball() * coordinate_frame_point(id);
  gl_Position = TS * vec4(vertex, 1);
  v_color = vec4(coordinate_frame_color(id), 1.);
}