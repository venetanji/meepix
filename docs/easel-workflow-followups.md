# Easel and creative-skills follow-up

Checked 2026-10-31 against the local Easel source and live endpoint. Sibling
repositories were inspected only; this PR changes MeePix.

## What works now

The live Easel model list includes `qwen-image-2.1`. One authorized 512x512 request
(requested 8 steps, seed 42) returned HTTP 202, progressed to completion and served
an authenticated 187876-byte PNG. No POST retry was performed. This establishes
Qwen text-to-image on this rig; it does not establish live reference-edit quality.
The new Rust client also exercises the full edit/multipart path against a fake
server. Qwen assets already serving this test need no additional download:

- diffusion model: `qwen_image_2.1_int8_convrot.safetensors`
- text encoder: `qwen3vl_8b_int8_convrot.safetensors`
- VAE: `qwen_image_2.1_vae_bf16.safetensors`

Existing `TextEncodeQwenImage21`, `QwenImage21Cache`, `JoinImageWithAlpha`, loader,
sampler and decoder nodes are reused. Qwen instruction edits are not masked
inpainting. Small outputs are generated at 512px and explicitly reduced with
nearest-neighbor sampling; no pixel-art or transparency quality guarantee is made.

## Missing API support

The editor's first workflow needs no new Easel route. Follow-ups that would help:

1. Add modality/capability metadata to `/v1/models`. It currently mixes image,
   video and audio IDs. MeePix exposes only its supported Qwen ID for now.
2. Add and test a MOSS backend/registry entry. MOSS is absent from both the local
   `MODELS` table and the live model list; downloading weights alone is insufficient.
3. Idempotency/reconciliation for interrupted POSTs and a supported cancellation
   endpoint would improve recovery. Neither is assumed by this client.
4. Receipt expiry is 24 hours. MeePix saves originals promptly, but needs a
   restart/recovery UI for stored jobs before expanding long-running workflows.

## MOSS download shortlist

Use the [Comfy-Org/MOSS-Image model repository](https://huggingface.co/Comfy-Org/MOSS-Image)
and its current model card. Start with the SFT model and **one** compatible text
encoder/VAE combination; do not download every precision variant:

| Component | File | ComfyUI destination |
| --- | --- | --- |
| SFT diffusion model | `moss_image_sft_int8_convrot.safetensors` | `models/diffusion_models/` |
| Text encoder | `qwen3.5_9b_int8_convrot.safetensors` | `models/text_encoders/` |
| VAE | `flux2-vae.safetensors` | `models/vae/` |
| Optional community edit adapter | `moss_image_edit_lora_rank_64_bf16.safetensors` | `models/loras/` |

The first three total roughly 15.5 GB on disk, not a VRAM requirement. BF16
alternatives are available when appropriate; Base is optional for experimentation.
The edit LoRA is a separate community workflow to validate, not evidence that
MOSS instruction editing is integrated. Verify its original model card and
adapter compatibility before selecting weights. Do not reuse the Qwen-Image-2.1
text encoder or VAE for MOSS.

The [upstream native ComfyUI guide](https://github.com/OpenMOSS/MOSS-Image/blob/main/comfyui/README.md)
specifies a version containing commit `e087d8f6` or later, current dependencies,
CLIP loader type `moss_image`, native layout prompt support and shift handling.
Use the upstream native workflow rather than swapping a loader into its old
plugin workflow. Model licenses remain separate from MeePix code.

## Workflows to implement in creative-skills, in order

These are proposals for a separate change, not implemented by this PR:

1. **Qwen sprite generation/reference edit:** preserve the existing API graph;
   add a reproducible seeded sprite brief, PNG reference preparation, RGBA handling,
   dimension reduction and palette evaluation. Compare accepted outputs at 16/32/64/128.
2. **MOSS SFT text-to-image:** port the official native workflow to a validated
   API graph, record node versions/assets and run a seeded smoke test. Add a distinct
   backend in Easel, register its model and expose capabilities only after success.
3. **MOSS layout-to-image:** preserve centered `yxyx` coordinates and matching
   generation dimensions. Test sprite-sheet layout rather than assuming usable,
   aligned animation frames from a contact sheet.
4. **MOSS edit LoRA experiment:** validate its own reference conditioning and
   settings separately; do not route Qwen reference payloads to MOSS unchanged.
5. **Acceptance/provenance recipe:** store receipt, original PNG, prompt/model,
   preprocessing and workflow version locally; keep previews separate and require
   explicit approval before replacing artwork.

Sources: [MOSS upstream](https://github.com/OpenMOSS/MOSS-Image),
[Comfy model card](https://huggingface.co/Comfy-Org/MOSS-Image/blob/main/README.md),
[Easel](https://github.com/venetanji/easel). These identify available assets and
workflow requirements; no MOSS weights were downloaded or inference tested here.
