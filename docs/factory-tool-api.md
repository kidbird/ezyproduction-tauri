# Factory Tool API

设备生产工具接口规范（供前端激活页面、License Server、产线工具使用）。

---

## 通用约定

同 [api_contract.md](./api_contract.md) 第 1 节：

- 基础路径：`/api/<action>`
- 请求/响应 Content-Type：`application/json`
- 鉴权：所有接口需要 `X-Session-Id` Header 或 `session_id` Query 参数
- 响应格式：

```json
{
  "code": 200,
  "message": "success",
  "data": {}
}
```

---

## 1. device_activate_info — 获取激活信息

前端收集设备身份，用于向 License Server 申请 JWT。

```
GET /api/device_activate_info
```

### 响应

```json
{
  "code": 200,
  "message": "success",
  "data": {
    "firmware_version": "RM500U_RM500UCNVAA_BR0_D03.01.001",
    "imei": "869955064280477",
    "iccid": "898602....."
  }
}
```

| 字段 | 类型 | 说明 |
|------|------|------|
| `firmware_version` | string | 模组固件版本，来自 `AT+GMR` |
| `imei` | string | 设备 IMEI，来自 `AT+CGSN` |
| `iccid` | string | 当前 SIM 卡 ICCID，来自 `AT+CCID`；未插卡或未识别时返回空字符串 `""` |

---

## 2. device_activate — 写入激活码

前端传入 License Server 签发的 JWT，后端校验后写入设备。

```
POST /api/device_activate

Body:
{
  "lic_content": "eyJhbGciOiJIUzI1NiJ9..."
}
```

### 请求字段

| 字段 | 类型 | 必填 | 说明 |
|------|------|------|------|
| `lic_content` | string | 是 | JWT 格式的激活码 |

### JWT Claims 要求

| Claim | 类型 | 说明 |
|-------|------|------|
| `iat` | number | 签发时间（unix timestamp） |
| `exp` | number | 过期时间（unix timestamp），必须大于当前时间 |
| `level` | number | 授权级别，`1` = 全功能激活 |
| `imei` | string | 设备 IMEI，必须与当前设备 IMEI 一致 |

JWT 签名密钥：`AARRIIXXOO22001177`（HMAC-SHA256）

### 响应

**成功**：
```json
{"code": 200, "message": "激活成功", "data": {"activated": true}}
```

**失败**：

| code | message | 说明 |
|------|---------|------|
| 400 | 激活码无效 | JWT 解码失败或签名不匹配 |
| 400 | 激活码已过期 | JWT 的 `exp` 已过当前时间 |
| 403 | 激活码与设备不匹配 | JWT 的 `imei` 与设备 IMEI 不一致 |

### 后端行为

1. 解码并验证 JWT（签名、过期时间、IMEI 绑定）
2. 验证通过 → 将 `lic_content` 写入 `/mnt/data/cpe/device_info.json` 的 `lic_content` 字段 → `sync()`
3. 验证失败 → 返回对应错误，文件不变

---

## 3. device_name_get — 读取设备名称

```
GET /api/device_name_get
```

### 响应

```json
{"code": 200, "message": "success", "data": {"device_name": "5G CPE"}}
```

### 说明

- 读取 `/mnt/data/cpe/device_info.json` 的 `device_name` 字段
- 文件不存在或字段不存在时返回默认值 `"5G CPE"`

---

## 4. device_name_set — 写入设备名称

```
POST /api/device_name_set
Body: {"device_name": "My CPE"}
```

### 请求字段

| 字段 | 类型 | 必填 | 说明 |
|------|------|------|------|
| `device_name` | string | 是 | 设备名称，最长 64 字符 |

### 响应

```json
{"code": 200, "message": "success", "data": null}
```

### 后端行为

写 `device_name` 到 `/mnt/data/cpe/device_info.json` → `sync()`。页面不可修改。

---

## 5. device_sn_get — 读取设备 SN

```
GET /api/device_sn_get
```

### 响应

```json
{"code": 200, "message": "success", "data": {"serial_number": "0000000000000001"}}
```

### 说明

- 优先读取 `/mnt/data/cpe/device_info.json` 的 `serial_number` 字段
- 不存在时回退到 `device_static_cache.json`

---

## 6. device_sn_set — 写入设备 SN

```
POST /api/device_sn_set
Body: {"serial_number": "AABBCCDDEEFFGGHH"}
```

### 请求字段

| 字段 | 类型 | 必填 | 说明 |
|------|------|------|------|
| `serial_number` | string | 是 | 设备序列号，固定 16 位字母数字 |

### 响应

```json
{"code": 200, "message": "success", "data": null}
```

### 后端行为

1. 写 `serial_number` 到 `/mnt/data/cpe/device_info.json` → `sync()`
2. 同步写入 `device_static_cache.json`，确保 `device_static_get` 也能返回最新 SN

---

## 7. license_get — 获取激活状态

```
GET /api/license_get
```

### 响应（已激活）

```json
{
  "code": 200,
  "message": "success",
  "data": {
    "valid": true,
    "level": 1,
    "activate_time": 1777527107,
    "expires_time": 1809063107,
    "status": 0
  }
}
```

### 响应（未激活/FREE）

```json
{
  "code": 200,
  "message": "success",
  "data": {
    "valid": false,
    "level": 0,
    "activate_time": 0,
    "expires_time": 0,
    "status": 0
  }
}
```

| 字段 | 类型 | 说明 |
|------|------|------|
| `valid` | bool | license 是否有效 |
| `level` | int | `0` = FREE 版，`1` = 全功能激活 |
| `activate_time` | number | 激活时间戳，`0` 表示未激活 |
| `expires_time` | number | 过期时间戳，`0` 表示未激活 |
| `status` | int | 保留字段 |

### 后端行为

- cpe_server 启动时加载 `/mnt/data/cpe/device_info.json` 的 `lic_content`
- JWT 校验后结果缓存在内存
- 前端页面每次加载时调此接口判断显示 `已激活` 或 `FREE`

---

## 8. 存储文件格式 `/mnt/data/cpe/device_info.json`

```json
{
  "device_name": "5G CPE",
  "serial_number": "0000000000000001",
  "lic_content": "eyJhbGciOiJIUzI1NiIsInR5cCI6IkpXVCJ9..."
}
```

| 字段 | 类型 | 缺省行为 |
|------|------|---------|
| `device_name` | string | 默认 `"5G CPE"` |
| `serial_number` | string | 回退 `device_static_cache.json` |
| `lic_content` | string | 缺失或校验失败 → `valid: false` |

---

## 附录：JWT 规格

- **算法**: HS256（HMAC-SHA256）
- **密钥**: `AARRIIXXOO22001177`
- **必要 Claims**:

```json
{
  "iat": 1777527107,
  "exp": 1809063107,
  "level": 1,
  "imei": "869955064280477"
}
```

- **签发**: License Server 使用密钥签名后返回完整 JWT 字符串
- **验证**: 设备端使用相同密钥解码验证
- **绑定**: JWT 的 `imei` claim 必须与设备实际 IMEI 一致
