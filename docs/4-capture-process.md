# USB Capture Process for MTP Test Fixtures

## What You're Capturing

MTP runs over USB bulk transfers. When you connect your phone and browse files, the conversation looks like:

```
Your Computer                          Phone
     │                                   │
     │──── "Open session please" ───────▶│
     │◀─── "OK, session open" ───────────│
     │                                   │
     │──── "List your storages" ────────▶│
     │◀─── "Internal: 64GB, SD: 32GB" ───│
     │                                   │
     │──── "List files in root" ────────▶│
     │◀─── "DCIM/, Download/, ..." ──────│
     │                                   │
     └───────────────────────────────────┘
```

You're recording both sides of this conversation as raw bytes.

---

## The Tools

### Option A: Wireshark (Recommended - GUI, cross-platform)

- Works on Linux, macOS, Windows
- Visual interface to see packets in real-time
- Can export to multiple formats
- On Linux: needs `usbmon` kernel module
- On macOS: needs additional setup but works
- On Windows: needs USBPcap

### Option B: usbmon + tcpdump (Linux command-line)

- Lower level, text-based
- Good for scripting
- Linux only

### Option C: Dedicated MTP sniffers

- Some exist but less common
- Often just wrappers around the above

---

## The Capture Session (Step by Step)

### 1. Preparation

- Close all file managers and apps that auto-mount MTP
- On Linux: stop `gvfs-mtp-volume-monitor` or similar
- You want a clean slate - no background MTP traffic

### 2. Start Capture

- Open Wireshark
- Select your USB bus (the one your phone will connect to)
- Start recording

### 3. Connect Phone

- Plug in USB cable
- Phone shows "USB connected" notification
- Select "File Transfer / MTP" mode on phone
- You'll see initial handshake packets appear in Wireshark

### 4. Perform Specific Operations

Do each operation **deliberately and one at a time** so you can label them later:

| Operation               | What to do                      | What it captures                     |
|-------------------------|---------------------------------|--------------------------------------|
| **Device detection**    | Just connect                    | GetDeviceInfo                        |
| **Open session**        | Let file manager connect        | OpenSession                          |
| **List storages**       | Open the device in file browser | GetStorageIDs, GetStorageInfo        |
| **List root folder**    | Click into Internal Storage     | GetObjectHandles, GetObjectInfo (×N) |
| **Navigate to folder**  | Click into DCIM                 | GetObjectHandles for that folder     |
| **Read file metadata**  | Select a file (don't open)      | GetObjectInfo, GetObjectPropList     |
| **Download small file** | Copy a small file to PC         | GetObject                            |
| **Upload small file**   | Copy a small text file to phone | SendObjectInfo, SendObject           |
| **Delete file**         | Delete that test file           | DeleteObject                         |
| **Close session**       | Safely eject / disconnect       | CloseSession                         |

### 5. Stop Capture

- Disconnect phone cleanly (eject first)
- Stop Wireshark recording
- Save the raw capture file (.pcapng)

---

## What the Raw Capture Looks Like

Wireshark shows you something like:

```
No.  Time     Source  Dest    Protocol  Info
1    0.000    host    1.2.1   USB       URB_BULK out
2    0.005    1.2.1   host    USB       URB_BULK in
3    0.006    host    1.2.1   USB       URB_BULK out
...
```

Each packet has raw bytes. For MTP, you'll see the container structure:

```
Frame 42: URB_BULK out (host → device)
  Raw: 10 00 00 00 01 00 02 10 01 00 00 00 01 00 00 00
       └─ length ─┘ └type┘ └code┘ └─ trans_id ─┘ └param1─┘

  Decoded: Command Container
           Length: 16 bytes
           Type: Command (0x0001)
           Code: OpenSession (0x1002)
           Transaction ID: 1
           Param1: 1 (session ID)
```

---

## Processing Captures into Test Fixtures

### Step 1: Filter to MTP-only traffic

USB captures include lots of noise (USB control transfers, other devices). Filter to:

- Your phone's USB address
- Bulk transfers only (that's where MTP data lives)

### Step 2: Group into request/response pairs

Each MTP transaction is:

```
Command (out) → [Data (in/out)] → Response (in)
```

You group these by transaction ID.

### Step 3: Extract and label

For each transaction, save:

- The command bytes
- Any data bytes
- The response bytes
- A human label ("GetStorageIDs", "ListRootFolder", etc.)

### Step 4: Normalize (optional)

Some things vary between captures:

- Transaction IDs (always incrementing)
- Timestamps in metadata
- Object handles (device assigns these)

You might want to note which fields are "variable" vs "fixed".

---

## Example Fixture Output

After processing, you'd have something like:

```
captures/
├── pixel6_session.json        # Full session from connect to disconnect
├── operations/
│   ├── open_session.json      # Just OpenSession request/response
│   ├── get_storage_ids.json   # GetStorageIDs
│   ├── list_root.json         # GetObjectHandles for root
│   └── download_file.json     # GetObject for a specific file
└── structures/
    ├── device_info.bin        # Raw DeviceInfo response payload
    ├── storage_info.bin       # Raw StorageInfo response payload
    └── object_info_mp3.bin    # Raw ObjectInfo for an MP3 file
```

Each JSON file might look like:

```json
{
  "description": "Open MTP session",
  "device": "Google Pixel 6",
  "transaction": {
    "command": {
      "hex": "10000000010002100100000001000000",
      "decoded": {
        "length": 16,
        "type": "Command",
        "code": "OpenSession",
        "transaction_id": 1,
        "params": [
          1
        ]
      }
    },
    "response": {
      "hex": "0c00000003000120010000",
      "decoded": {
        "length": 12,
        "type": "Response",
        "code": "OK",
        "transaction_id": 1
      }
    }
  }
}
```

---

## Safety Notes

| Concern                  | Risk Level   | Mitigation                                                                          |
|--------------------------|--------------|-------------------------------------------------------------------------------------|
| Capturing damages phone  | **None**     | You're just observing USB traffic                                                   |
| Uploading corrupts data  | **Very low** | Only upload a test file you create, then delete it                                  |
| Private data in captures | **Medium**   | Filenames, folder structure visible in captures - don't share raw captures publicly |
| Phone left in bad state  | **Very low** | Always cleanly eject before disconnecting                                           |

---

## Recommended Capture Plan

### Session 1: Basic discovery (read-only, safest)

1. Connect
2. Let it enumerate storages
3. Browse to DCIM
4. Browse to a subfolder
5. Disconnect cleanly

### Session 2: File operations (minimal writes)

1. Connect
2. Navigate to Download folder
3. Copy a tiny test.txt (10 bytes) TO the phone
4. Read it back
5. Delete it
6. Disconnect

### Session 3: Edge cases (if needed later)

- Large file transfer (to test chunking)
- File with unicode name
- Deep folder navigation

---

## Time Estimate

| Task                                   | Time      |
|----------------------------------------|-----------|
| Install Wireshark + USB capture setup  | 15-30 min |
| Capture session 1 (read-only)          | 10 min    |
| Capture session 2 (with upload/delete) | 10 min    |
| Process captures into fixtures         | 1-2 hours |

After that initial investment, you never need the device again for testing.
