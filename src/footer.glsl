/*
GRIMOIRE BEGIN: default_shader_footer.glsl
*/
#if defined(GRIM_VERTEX) && !defined(GRIM_OVERRIDE_MAIN) && !defined(GRIM_OVERRIDE_VERTEX_MAIN)
void main() {
    float x = -1.0 + float((gl_VertexID & 1) << 2);
    float y = -1.0 + float((gl_VertexID & 2) << 1);
    gl_Position = vec4(x, y, 0, 1);
}
#endif

#if defined(GRIM_FRAGMENT) && !defined(GRIM_OVERRIDE_MAIN) && !defined(GRIM_OVERRIDE_FRAGMENT_MAIN)
out vec4 GRIM_FRAG_COLOR;
void main() {
  mainImage(GRIM_FRAG_COLOR, gl_FragCoord.xy);
}
#endif
/*
GRIMOIRE END: default_shader_footer.glsl
*/

