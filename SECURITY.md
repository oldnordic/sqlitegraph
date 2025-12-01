# Security Policy

## Supported Versions

| Version | Supported          |
|---------|--------------------|
| 0.1.x   | ✅ Current         |
| < 0.1   | ❌ Unsupported     |

## Reporting a Vulnerability

Please keep security vulnerabilities confidential until they have been responsibly disclosed.

### Responsible Disclosure Process

1. **Do not open a public issue** - Security vulnerabilities should be reported privately
2. **Report through GitHub Issues** - Open a GitHub issue with the "Security" label and include "SECURITY:" in the title
3. **Provide details** - Include:
   - Affected version(s)
   - Steps to reproduce
   - Potential impact assessment
   - Any proposed mitigations (if known)

### Response Timeline

- **Initial response**: Within 7 days
- **Assessment**: Within 14 days  
- **Fix release**: As soon as practicable, based on severity

### GPL-3.0-or-later Compliance

This project is licensed under GPL-3.0-or-later. Security fixes and patches must comply with these license terms. All security-related contributions will be accepted under the same license.

### What to Report

Report vulnerabilities that could allow:
- Unauthorized data access
- Code execution
- Denial of service
- Privilege escalation
- Data corruption

### Out of Scope

Do not report:
- Dependencies vulnerabilities (report to upstream projects)
- Theoretical vulnerabilities without reproduction steps
- Issues requiring physical access to hardware

## Security Best Practices

Users should:
- Keep dependencies updated
- Review database permissions
- Use input validation for external data
- Monitor access logs in production environments

### Contact

For security matters, please open a GitHub issue with "SECURITY:" in the title. This is the preferred contact method for this solo-maintained open-source project.