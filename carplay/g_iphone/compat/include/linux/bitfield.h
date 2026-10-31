/* compat (4.4): minimal FIELD_PREP/FIELD_GET (no compile-time checks) */
#pragma once
#define FIELD_PREP(_mask, _val) (((typeof(_mask))(_val) << __builtin_ctzll(_mask)) & (_mask))
#define FIELD_GET(_mask, _reg) ((typeof(_mask))(((_reg) & (_mask)) >> __builtin_ctzll(_mask)))
