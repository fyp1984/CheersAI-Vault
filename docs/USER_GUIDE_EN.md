# CheersAI Desensitization Sandbox User Manual

## 1. Purpose and Intended Audience

This manual helps enterprise users, individual users, deployment administrators, and support engineers understand and use CheersAI Desensitization Sandbox.

The document is intended for the following audiences:

- Office users who need to remove sensitive information before sharing files externally
- Individuals and teams who need to clean content before sending it to AI agents or large language models
- Managers who need to implement a repeatable data-masking process inside an organization
- Technical staff responsible for installation, deployment, operations, and support

The following reading conventions are used in this manual:

- UI labels, button names, and API paths are shown in code style
- “Optional” means the capability is not required by default
- “Administrator” refers to the person responsible for enterprise deployment, OCR components, FileBay configuration, and runtime maintenance

## 2. Software Overview

### 2.1 Product Positioning

CheersAI Desensitization Sandbox is an open source local file-masking application for enterprise teams and individual users. It is designed to remove sensitive data such as names, phone numbers, ID numbers, email addresses, addresses, customer data, and project codenames before files are shared externally, circulated for review, or submitted to AI agents and large language models. After the task is completed, the same local mapping rules can be used to restore the required content with one click.

### 2.2 Editions and Deployment Modes

| Mode | Primary Users | Typical Scenario | Notes |
|---|---|---|---|
| Desktop edition | Individual users and local office users | Local processing, offline handling, local rule management | Built with Tauri |
| Browser edition | Enterprise intranet users | Shared team access, centralized deployment, centralized operations | Served by Nginx and Runtime |
| Local Docker validation environment | Developers and testers | Fast integration validation for the browser workflow | Intended for local validation, not the same as a formal production rollout |

### 2.3 Typical Workflow

```text
Select files
  ↓
Choose masking rules or a sensitive-term library
  ↓
Generate a masking preview
  ↓
Confirm and generate final output
  ↓
Share externally or use with AI
  ↓
Run restore when the original content is needed again
```

## 3. Functional Overview

### 3.1 Core Capability Matrix

| Capability | Desktop Edition | Browser Edition | Notes |
|---|---|---|---|
| Single-file masking | Supported | Supported | Suitable for ad hoc file handling |
| Batch masking | Supported | Supported | Suitable for outbound file packages or archive preparation |
| Masking preview | Supported | Supported | Review the result before generating the final output |
| File restore | Supported | Supported | Restore the original content based on mapping rules |
| Sensitive-term library management | Supported | Supported | Add, edit, enable, disable, import, and export |
| Sandbox and PIN protection | Supported | Supported | Protects the controlled workspace and critical operations |
| FileBay integration | Supported | Supported | Used for uploading masked result files |
| OCR capability | Optional | Optional | Primarily used for scanned PDF text extraction |
| AI multi-method detection | Supported | Depends on deployment | Used to improve detection coverage; the current UI is authoritative |

### 3.2 Supported File Types

The software currently supports the following common file types:

- `CSV`
- `Excel`: `.xlsx`, `.xls`
- `JSON`
- `TXT`
- `Word`: `.docx`
- `PowerPoint`: `.pptx`, with enterprise runtime support for legacy `.ppt`
- `PDF`
- `Markdown`

Please note the following:

- Scanned PDFs require OCR support
- Some detailed behaviors may vary by runtime mode and deployment configuration; the current release UI and deployment profile remain authoritative

## 4. System Requirements

### 4.1 End-User Environment

| Scenario | Basic Requirement | Notes |
|---|---|---|
| Desktop use | Installed desktop application or a source-build environment | Best for individual or local office users |
| Browser use | Modern web browser and enterprise intranet access | The browser edition does not install OCR locally |
| Scanned PDF processing | OCR runtime configured by an administrator | Without OCR, text cannot be extracted from image-only PDFs |

### 4.2 Build and Deployment Environment

| Role | Baseline Requirement |
|---|---|
| Desktop source build | `Node.js 22+`, `pnpm 11+`, `Rust 1.85+` |
| OCR management | `Python 3.11+`, required only for OCR scenarios |
| Local Docker validation | Docker and Docker Compose |
| Enterprise intranet deployment | Linux host, Nginx, systemd, and the Runtime environment |

## 5. Installation and Deployment Guide

### 5.1 Desktop Source Run

This mode is suitable for development, testing, and local debugging.

1. Install the required dependencies:
   - `Node.js 22+`
   - `pnpm 11+`
   - `Rust 1.85+`
2. Run the following commands at the repository root:

```bash
pnpm install --frozen-lockfile
pnpm build
pnpm tauri dev
```

3. To build installable packages, use the commands for the target platform:

```bash
pnpm tauri build
pnpm build:windows
pnpm build:linux
```

Additional note for local development:

- If local debugging needs to reuse FileBay settings from the CheersAI Desktop online workspace, sign in there first, complete the configuration sync, and then return to this software to refresh FileBay status
- This path is a development convenience only and is not part of the standard enterprise deployment flow

### 5.2 Local Docker Validation

This mode is intended for fast validation of the browser frontend and Runtime integration.

1. Run the following command at the repository root:

```bash
docker compose up -d --build
```

2. Open the following URLs to verify service availability:
   - Browser entry: `http://127.0.0.1:5173`
   - Health endpoint: `http://127.0.0.1:5173/api/v1/health`

3. To stop the environment:

```bash
docker compose down
```

### 5.3 Enterprise Intranet Deployment

This mode is suitable for a centrally managed browser entry inside an enterprise network.

The high-level procedure is as follows:

1. Build the Runtime:

```bash
cargo build --release --manifest-path apps/vault-runtime-api/Cargo.toml
```

2. Build the browser frontend assets:

```bash
pnpm install --frozen-lockfile
pnpm exec vite build
```

3. Configure Runtime environment variables, OCR components, and FileBay settings
4. Manage the Runtime process with systemd
5. Use Nginx to serve the frontend and reverse proxy `/api`
6. Run post-deployment connectivity and smoke checks

For detailed enterprise deployment procedures, see:

- [`docs/enterprise/DEPLOYMENT.md`](./enterprise/DEPLOYMENT.md)
- [`deploy/linux/README.md`](../deploy/linux/README.md)

### 5.4 Uninstallation and Cleanup Recommendations

Before uninstalling or cleaning up an environment, complete the following steps:

- Back up masked outputs and mapping files that still need to be retained
- Export sensitive-term libraries that still need to be preserved
- Revoke or rotate FileBay access tokens if FileBay integration is in use
- In enterprise environments, stop Runtime and Nginx before removing binaries, static assets, and runtime directories

## 6. Core Feature Operation Guide

### 6.1 Quick Start

Use the following sequence for a recommended first run:

1. Open the software or browser entry
2. Review rules and the sensitive-term library
3. Upload one file or a batch of files
4. Select the required rules
5. Generate a masking preview
6. Confirm and generate the final output
7. Share the masked file or send the masked content to AI
8. Run file restore when the original content is needed again

### 6.2 File Masking

File masking is used to generate output that can be safely shared, circulated, or submitted to AI systems.

Use the following steps:

1. Open `文件脱敏` or the equivalent masking page
2. Select one or more input files
3. Enable the built-in rules that should be applied
4. Enable the sensitive-term library if needed
5. Select an output location or confirm the server-side workflow
6. Click `生成脱敏预览` or the equivalent preview action
7. Review the preview
8. Click `确认并生成正式批次` or the equivalent confirmation action

Recommended practices:

- Always review a preview before sharing the result externally
- For large batches, validate the rule set with a small sample first
- Combine the sensitive-term library with enhanced detection if broader coverage is required

Additional notes for page-range processing:

- For `PDF`, `Word (.docx)`, and `PowerPoint (.pptx)` files, the system can display the total page count and process only a selected page range
- Common inputs include `1-10` and `5-15`; leaving the field blank means the whole file will be processed
- Page ranges are useful for quick sampling, chapter-level processing, and splitting very large files into smaller runs
- Output files include the selected range in the file name, for example `filename_masked_p1-10.txt`

Additional notes for preview-stage quick actions:

- In the manual find-and-replace area, a detected entity can be added to the replacement list by clicking the badge text directly
- When the pointer hovers over a badge, the upper-left quick-action button adds that entity directly to the replacement list
- The upper-right delete button removes a false-positive detection
- Once an entity is added or removed manually, it is automatically hidden from the current detected-entity area

### 6.3 File Restore

File restore is used to recover original content after editing, approval, or AI-assisted work is completed.

Use the following steps:

1. Open `文件反脱敏` or the equivalent restore page
2. Select the masked output file
3. Select the matching mapping file, or reference the stored mapping data in the system
4. Enter the correct password if the mapping file is encrypted
5. Select an output location
6. Click `开始反脱敏` or the equivalent restore action

Important notes:

- The mapping file must match the masked output
- Restore is not possible if the mapping file is missing, corrupted, or protected by an unknown password
- Restored files should be treated as sensitive content and handled accordingly

### 6.4 Rules and Sensitive-Term Library

The rule system combines built-in rules with a customizable sensitive-term library.

Common built-in rules include:

- ID number
- Phone number
- Email address
- Bank card number
- IPv4 address
- Passport number

The sensitive-term library supports the following operations:

- Add entries
- Edit entries
- Enable or disable entries
- Organize by category
- Import from CSV
- Export to CSV

Recommended practices:

- Maintain company names, customer names, project codenames, place names, and organization names by category
- Validate replacement behavior with sample files before formal batch processing
- Update the library regularly for high-frequency business content

Additional notes for list management:

- The sensitive-term list supports search, category filters, pagination, and sorting
- Sorting can be switched between creation time and alphabetical order
- When the list grows large, filter by category first and then search or edit in batches
- High-frequency business terms that are not a good fit for regex rules should be maintained in the sensitive-term library instead

### 6.5 Sandbox and PIN

The sandbox protects a controlled workspace on the local machine or on the server.

Common actions include:

- Set a shared or local PIN
- Unlock the sandbox
- Lock the sandbox
- Clear the PIN

Recommended practices:

- Set a memorable but sufficiently strong PIN when enabling the feature
- Do not store the PIN together with mapping files in a public location
- In the browser edition, sandbox state is typically maintained by the server, and sessions under the same service instance may share that state

### 6.6 FileBay Upload

FileBay is used to store masked result files. It should not be used to upload original sensitive files.

Use the following steps:

1. The administrator configures the FileBay server, token, and target repository
2. The user opens `FileBay 设置` or the relevant upload entry
3. The user verifies the connection status
4. The user selects masked outputs from completed jobs
5. The user confirms the target repository and remote path
6. The user submits the upload

Important notes:

- The upload target is usually configured centrally by an administrator
- Mapping files and original files should not be uploaded with masked outputs
- If the status is shown as “unconfigured” or “invalid,” contact the administrator

### 6.7 Enhanced Services

Enhanced services mainly include OCR and optional enhanced detection capabilities.

The general rules are:

- In the desktop edition, installation and checks may be performed locally by the user or administrator
- In the browser edition, the page usually shows read-only server status, and installation or repair is handled by an administrator
- Without OCR, scanned PDFs cannot be processed correctly

### 6.8 Operation Logs

Operation logs help users track job status, processing results, and failure reasons.

Common user actions include:

- Filter by status
- Search by job or batch identifier
- View job details
- Review failure reasons
- Retry failed items

Administrators should pay special attention to the following:

- Repeated failures across many jobs
- Frequent OCR timeouts
- Persistent FileBay upload failures
- Unhealthy Runtime status

## 7. Troubleshooting Guide

### 7.1 Common Symptoms and Recommended Actions

| Symptom | Possible Cause | Recommended Action |
|---|---|---|
| The page reports that Runtime is unavailable | Runtime is not running, Nginx proxying is incorrect, or the network is temporarily interrupted | Check the health endpoint first, then contact the administrator |
| A scanned PDF cannot be recognized | OCR is not installed or not available | Have the administrator install or repair the OCR component |
| A Chinese PDF reports a font-encoding problem or plain extraction fails | The PDF uses a special font encoding and normal text extraction fails | Retry the preview first; if OCR is available, the system can fall back automatically |
| A legacy `.ppt` file cannot be processed | LibreOffice conversion is not available | Have the administrator verify the LibreOffice installation |
| Restore fails | The mapping file does not match, the password is incorrect, or the mapping file is corrupted | Verify the file pairing and password |
| FileBay upload fails | FileBay is not configured, the token has expired, or the repository is unreachable | Contact the administrator to review the FileBay configuration |
| The preview does not match the final result | Rules changed, the sensitive-term library changed, or the job was resubmitted | Reconfirm the active rule set and the target batch |
| File content is masked but sensitive words in the file name are unchanged | The sensitive-term library was not enabled, the term is not active, or the current file name does not match the library entry | Confirm that the batch used the sensitive-term library and that the relevant entry is enabled |

### 7.2 Administrator Checklist

Administrators can troubleshoot in the following order:

1. Check the Runtime health endpoint: `GET /api/v1/health`
2. Check whether the frontend entry and `/api` reverse proxy are working correctly
3. Check the OCR interpreter, script path, and model directory
4. Check whether LibreOffice can be invoked
5. Check whether all four FileBay settings are complete and active after restart
6. Check runtime directory permissions, logs, and persistent storage

For more deployment-level troubleshooting, see:

- [`docs/enterprise/DEPLOYMENT.md`](./enterprise/DEPLOYMENT.md)
- [`SECURITY.md`](../SECURITY.md)

## 8. Compliance, Security, and Privacy Statement

The software follows the principles below:

- Sensitive files should be processed locally or within the enterprise intranet whenever possible
- Mapping files and passwords are used to protect restore capability
- Original sensitive files should not be exposed directly to external sharing channels or external AI services
- The recommended workflow is to mask first, then share or collaborate

The compliance boundary is as follows:

- The main repository license is `Apache-2.0`
- Third-party license obligations still apply independently
- OCR-related dependencies require additional license review, especially `PyMuPDF`
- If OCR-enabled images, installers, or customer deliverables are redistributed, an additional license review must be completed first

For details, see:

- [`LICENSE`](../LICENSE)
- [`NOTICE`](../NOTICE)
- [`DEPENDENCIES.md`](../DEPENDENCIES.md)
- [`SECURITY.md`](../SECURITY.md)

## 9. Copyright and Open Source Notice

The copyright and open source details are as follows:

- Software name: CheersAI Desensitization Sandbox
- Repository identifier: `cheersai-vault`
- Copyright statement: `Copyright 2024-2026 CheersAI contributors`
- Main license: `Apache License 2.0`

Distribution requirements include:

- The license text must be included when redistributing the software
- If the distribution includes NOTICE requirements, the corresponding notices must also be retained
- Modified files should preserve change notices and copyright information as required by Apache 2.0

## 10. Technical Support and Feedback Channels

The public support channels are:

- Product home and code hosting: The project is hosted on a public GitHub repository under the name `CheersAI-Vault`; please use the repository URL recorded in your customer internal documentation as the single source of truth.
- Issue tracking: Submit issues and reproduction details through the Issues channel of the code repository.
- Code contribution path: Pull Request

The security reporting channels are:

- Use private vulnerability reporting or Security Advisories on the code hosting platform whenever available
- If private reporting is not enabled, use the maintainer's private contact channel disclosed on the hosting platform
- Review the handling scope and principles in [`SECURITY.md`](../SECURITY.md)

When submitting a support request, include the following whenever possible:

- Software version or commit identifier
- Runtime mode: desktop, browser, or Docker validation
- Reproduction steps
- Error message or screenshot
- Whether OCR, FileBay, mapping files, or enterprise intranet deployment is involved

## Appendix A. Visual Asset and Screenshot Synchronization Checklist

To keep the Chinese and English manuals aligned across desktop and mobile reading scenarios, the following figures and screenshots should be synchronized in the official release package. If no approved screenshots exist in the repository, capture them again from the current production UI instead of reusing outdated UI assets.

| Figure ID | Recommended Title | Recommended Content | Synchronization Requirement |
|---|---|---|---|
| A-1 | Main interface overview | Home page or primary navigation | Use the same page structure and the same product version in both language editions |
| A-2 | File masking workflow | File selection, rule selection, preview generation | Capture the same workflow and replace the UI language only |
| A-3 | File restore workflow | Select masked file, mapping file, and start restore | Keep the same sequence and field layout |
| A-4 | Sensitive-term library management | Add, import, filter, and export operations | Keep fields, buttons, and states aligned one to one |
| A-5 | Sandbox and PIN settings | Lock, unlock, and PIN management | Use the same security state example |
| A-6 | FileBay upload confirmation | Target repository, selected files, confirm upload | Do not expose real tokens or sensitive URLs |
| A-7 | Enhanced services status | OCR status and component readiness | Capture the actual current UI for desktop and browser separately |
| A-8 | Typical error examples | Runtime unavailable, OCR unavailable, upload failure | Do not include real customer data, real paths, or real tokens |
