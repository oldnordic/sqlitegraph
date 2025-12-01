# SQLiteGraph Changelog

## [Unreleased]

### Internal: Dead Code Audit Completed
A full audit of all clippy `dead_code` warnings was performed:

- 149 warnings flagged  
- 149 confirmed as false positives  
- 0 unused or obsolete items found  

Warnings come from:
- CLI modules  
- benchmark tooling  
- dual-runtime system  
- tests  
- DSL/pipeline parsers  

No code removed and no suppressions added. Documentation updated accordingly.