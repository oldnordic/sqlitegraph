# Splice Limitations - Enum Variant Modifications

**Date**: 2024-12-23
**Tool**: Splice v0.1.0
**Repository**: https://github.com/oldnordic/splice

---

## LIMITATION IDENTIFIED

**Issue**: Splice cannot target individual enum variants for patching.

**Error Message**:
```
Error: Symbol not found: EdgeInsert in sqlitegraph/src/backend/native/v2/wal/recovery/replayer/types.rs
```

**Command Attempted**:
```bash
splice patch \
  --file sqlitegraph/src/backend/native/v2/wal/recovery/replayer/types.rs \
  --symbol EdgeInsert \
  --kind enum \
  --with /tmp/edgeinsert_variant_only.rs \
  --verbose
```

---

## ROOT CAUSE

Splice's symbol resolution works at the **function level**, not at the **enum variant level**.

When targeting an enum:
- ✅ Can find `enum RollbackOperation` (the type)
- ❌ Cannot find `EdgeInsert` (individual variant)

**Underlying Issue**: Tree-sitter parses enum variants as struct-like patterns within the enum, not as separate named symbols that can be individually resolved and replaced.

---

## WORKAROUND USED

For the EdgeInsert rollback fix, we used **manual Edit tool** instead of Splice:

```bash
# Traditional edit instead of Splice
Edit file_path="/path/to/types.rs" old_string="..." new_string="..."
```

**Modified Files** (using Edit tool, not Splice):
1. `types.rs:108-114` - EdgeInsert enum variant
2. `operations.rs:501-578` - Rollback operation creation timing
3. `rollback.rs:105-107` - Pattern match update
4. `rollback.rs:390-420` - Function signature update
5. `rollback.rs:812-818` - Test constructor 1
6. `rollback.rs:1021-1027` - Test constructor 2
7. `types.rs:284-290` - Test constructor 3

---

## IMPACT ASSESSMENT

### What Splice CAN Do ✅

1. **Function-level patches** - Complete function replacement
   - Target: `--symbol function_name --kind function`
   - Works perfectly for:
     - `fn handle_edge_insert(...)`
     - `fn rollback_edge_insert(...)`
     - Any standalone function

2. **Method patches** - Struct/enum impl methods
   - Target: `--symbol method_name --kind function` (method is also function-level)
   - Works for impl blocks

3. **Struct/enum definition patches** - Replace entire type
   - Target: `--symbol TypeName --kind struct` or `--kind enum`
   - Would replace the ENTIRE enum definition

### What Splice CANNOT Do (Currently) ❌

1. **Enum variant patches** - Individual variant modification
   - Target: `--symbol VariantName --kind enum`
   - **Fails**: Cannot resolve individual variants
   - **Impact**: Must use Edit tool or manual modification

2. **Struct field patches** - Individual field modification
   - Likely similar limitation (not tested)

3. **Pattern match arm patches** - Individual match arm
   - Must target entire function containing match expression

---

## POTENTIAL ENHANCEMENTS FOR SPLICE

### Option 1: Add Variant-Level Targeting

**New Flag**: `--variant <variant_name>`

```bash
splice patch \
  --file types.rs \
  --symbol RollbackOperation \
  --kind enum \
  --variant EdgeInsert \
  --with /tmp/edgeinsert_variant_only.rs
```

**Implementation**:
1. Parse enum definition
2. Locate specific variant
3. Replace only that variant
4. Preserve surrounding variants

**Complexity**: Medium - Requires tree-sitter enum traversal

### Option 2: Struct Field-Level Targeting

**New Flag**: `--field <field_name>`

Similar approach for struct fields.

### Option 3: Match Arm Targeting

**New Flag**: `--match-arm <pattern>`

Target individual match arms within functions.

---

## RECOMMENDATION

### Short Term (Current)

1. Use **Edit tool** for enum variant modifications
2. Continue using **Splice for function-level** changes
3. Document this limitation in user manual

### Medium Term

1. **Add variant-level support** to Splice
2. Add `--variant` flag for enum patches
3. Update tree-sitter queries to support enum variant resolution

### Long Term

1. Support **finer-grained targeting**:
   - Struct fields
   - Match arms
   - Expression statements
   - Individual statements in blocks

2. **Targeted span selection**:
   - Line-based selection
   - AST node selection
   - Multi-statement selection

---

## FEASIBILITY ASSESSMENT

### Adding Enum Variant Support

**Technical Feasibility**: ✅ **HIGH**

**Why**:
1. Tree-sitter already parses enum variants
2. Variants have clear AST node types
3. Span calculation is straightforward
4. Can be implemented as special case of enum patching

**Implementation Steps**:
1. Modify `tree_sitter_query` to locate enum variants
2. Add `--variant` flag to CLI arguments
3. Update patch generation to replace only target variant
4. Add validation to ensure surrounding variants remain intact

**Estimated Effort**: 4-8 hours for basic variant targeting

**Benefits**:
- Enables precise enum modifications
- Maintains consistency with existing Splice workflow
- Reduces need for manual edits
- Improves safety (AST-validated)

---

## WORKFLOW COMPARISON

### Before (Manual Edit)

```bash
# 1. Read file to find variant
rg "EdgeInsert {" types.rs -A 5

# 2. Manual Edit
Edit tool with old_string/new_string

# 3. Verify compilation
cargo check

# 4. Update all usage sites manually
# (Repeat Edit for each location)
```

**Time**: 30-60 minutes for 7 files

### After (Splice with Variant Support)

```bash
# 1. Create replacement variant
cat > /tmp/edgeinsert_variant.rs << 'EOF'
    EdgeInsert {
        cluster_key: (u64, u64),
        insertion_point: u32,
        edge_record: Vec<u8>,
        cluster_offset: u64,
        cluster_size: u32,
    },
EOF

# 2. Apply Splice patch
splice patch \
  --file types.rs \
  --symbol RollbackOperation \
  --kind enum \
  --variant EdgeInsert \
  --with /tmp/edgeinsert_variant.rs \
  --verbose

# 3. Splice automatically validates and updates usage sites!
```

**Time**: 5-10 minutes total

**Time Savings**: 80-90%

---

## USE CASES FOR VARIANT-LEVEL PATCHING

1. **Adding enum variant fields** (our use case)
   - Add `cluster_offset: u64` to `EdgeInsert`
   - Add `cluster_size: u32` to `EdgeInsert`

2. **Renaming variants**
   - `NodeInsert` → `NodeCreate`
   - Preserve discriminant values

3. **Reordering variants**
   - Change variant declaration order
   - Maintains semantic compatibility

4. **Adding new variants**
   - Insert new variant in specific position
   - Avoid disrupting existing code

5. **Removing variants**
   - Deprecate unused variants
   - Maintain backward compatibility

---

## CONCLUSION

**Current State**: Splice is excellent for function-level patches but cannot modify individual enum variants.

**Workaround**: Use Edit tool for enum modifications (functional but less automated).

**Recommendation**: Add variant-level targeting to Splice as medium-priority enhancement.

**Impact**: Low priority for most use cases (functions are more common), but high value for enum-heavy codebases.

---

**Documented**: 2024-12-23
**Splice Version**: 0.1.0
**Context**: EdgeInsert rollback structure fix in SQLiteGraph
