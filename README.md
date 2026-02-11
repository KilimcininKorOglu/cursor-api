# cursor-api

## Description

* The current version is stable. If you encounter missing characters in responses, it is not related to this program.
* If you experience slow first character response, it is not related to this program.
* If you encounter garbled responses, it is also not related to this program.
* For issues related to the official service, please do not report them to the author.
* This program has performance comparable to the original client, possibly even faster.
* The performance of this program is excellent.
* According to the open source license of this project, forked projects cannot promote, advertise, or make statements in the author's name. Please use it discreetly.
* Updates have been ongoing for nearly 10 months. Sponsorship is welcome, but the project is free and no customization is offered.
* Self-deployment is recommended. The [official website](https://cc.wisdgod.com) is only for author testing and stability is not guaranteed.

## Getting the Key

1. Visit [www.cursor.com](https://www.cursor.com) and complete registration/login
2. Open developer tools in your browser (F12)
3. In Application-Cookies, find the entry named `WorkosCursorSessionToken` and copy its third field. Note that %3A%3A is the URL-encoded form of ::, and the cookie value is separated by colons (:).

## Configuration

### Environment Variables

* `PORT`: Server port number (default: 3000)
* `AUTH_TOKEN`: Authentication token (required for API authentication)
* `ROUTE_PREFIX`: Route prefix (optional)

For more details, see `/env-example`

### Token File Format (Deprecated)

`.tokens` file: Each line contains a token and checksum pair:

```
# The # here indicates this line will be deleted on next read
token1,checksum1
token2,checksum2
```

This file can be automatically managed, but users should only modify it if they are confident in their ability to do so. Manual modification is generally only needed in the following cases:

* Need to delete a specific token
* Need to use an existing checksum for a specific token

### Model List

The model list is hardcoded and custom model lists will not be supported in the future, as dynamic updates are already supported. See [Model List Update Instructions](#model-list-update-instructions) for details.

Check the program itself for the actual list.

## API Documentation

### Basic Chat

* Endpoint: `/v1/chat/completions`
* Method: POST
* Authentication: Bearer Token
  1. Using the `AUTH_TOKEN` environment variable
  2. Using dynamic keys built via `/build-key`
  3. Using shared tokens set via `/config` (related: `SHARED_TOKEN` environment variable)
  4. Using cached token key representations from logs (`/build-key` also provides these two formats as aliases for dynamic keys; the numeric key is essentially a 192-bit integer)

#### Request Format

```json
{
  "model": string,
  "messages": [
    {
      "role": "system" | "user" | "assistant", // "system" can also be "developer"
      "content": string | [
        {
          "type": "text" | "image_url",
          "text": string,
          "image_url": {
            "url": string
          }
        }
      ]
    }
  ],
  "stream": bool,
  "stream_options": {
    "include_usage": bool
  }
}
```

#### Response Format

If `stream` is `false`:

```json
{
  "id": string,
  "object": "chat.completion",
  "created": number,
  "model": string,
  "choices": [
    {
      "index": number,
      "message": {
        "role": "assistant",
        "content": string
      },
      "finish_reason": "stop" | "length"
    }
  ],
  "usage": {
    "prompt_tokens": 0,
    "completion_tokens": 0,
    "total_tokens": 0
  }
}
```

If `stream` is `true`:

```
data: {"id":string,"object":"chat.completion.chunk","created":number,"model":string,"choices":[{"index":number,"delta":{"role":"assistant","content":string},"finish_reason":null}]}

data: {"id":string,"object":"chat.completion.chunk","created":number,"model":string,"choices":[{"index":number,"delta":{"content":string},"finish_reason":null}]}

data: {"id":string,"object":"chat.completion.chunk","created":number,"model":string,"choices":[{"index":number,"delta":{},"finish_reason":"stop"}]}

data: [DONE]
```

### Get Model List

* Endpoint: `/v1/models`
* Method: GET
* Authentication: Bearer Token

#### Query Parameters

Optional JSON request body for model list parameters:

```json
{
  "is_nightly": bool,                    // Whether to include nightly version models, default false
  "include_long_context_models": bool,   // Whether to include long context models, default false  
  "exclude_max_named_models": bool,      // Whether to exclude max-named models, default true
  "additional_model_names": [string]     // Additional model names to include, default empty array
}
```

**Note**: Authentication is optional. Query parameters are optional and only take effect when authenticated. Default values are used when not provided.

#### Response Format

```typescript
{
  object: "list",
  data: [
    {
      id: string,
      display_name: string,
      created: number,
      created_at: string,
      object: "model",
      type: "model", 
      owned_by: string,
      supports_thinking: bool,
      supports_images: bool,
      supports_max_mode: bool,
      supports_non_max_mode: bool
    }
  ]
}
```

#### Model List Update Instructions

The latest model list is fetched each time a token is provided, with at least 30 minutes between updates. `additional_model_names` can be used to add extra models.

### Token Management Endpoints

#### Get Token Information

* Endpoint: `/tokens/get`
* Method: POST
* Authentication: Bearer Token
* Response Format:

```typescript
{
  status: "success",
  tokens: [
    [
      number,
      string,
      {
        primary_token: string,
        secondary_token?: string,
        checksum: {
          first: string,
          second: string,
        },
        client_key?: string,
        config_version?: string,
        session_id?: string,
        proxy?: string,
        timezone?: string,
        gcpp_host?: "Asia" | "EU" | "US",
        user?: {
          user_id: int32,
          email?: string,
          first_name?: string,
          last_name?: string,
          workos_id?: string,
          team_id?: number,
          created_at?: string,
          is_enterprise_user: bool,
          is_on_new_pricing: bool,
          privacy_mode_info: {
            privacy_mode: "unspecified" | "no_storage" | "no_training" | "usage_data_training_allowed" | "usage_codebase_training_allowed",
            is_enforced_by_team?: bool
          }
        },
        status: {
          enabled: bool
        },
        usage?: {
          billing_cycle_start: string,
          billing_cycle_end: string,
          membership_type: "free" | "free_trial" | "pro" | "pro_plus" | "ultra" | "enterprise",
          limit_type: "user" | "team",
          is_unlimited: bool,
          individual_usage: {
            plan?: {
              enabled: bool,
              used: int32,
              limit: int32,
              remaining: int32,
              breakdown: {
                included: int32,
                bonus: int32,
                total: int32
              }
            },
            on_demand?: {
              enabled: bool,
              used: int32,
              limit?: int32,
              remaining?: int32
            }
          },
          team_usage: {
            plan?: {
              enabled: bool,
              used: int32,
              limit: int32,
              remaining: int32,
              breakdown: {
                included: int32,
                bonus: int32,
                total: int32
              }
            },
            on_demand?: {
              enabled: bool,
              used: int32,
              limit?: int32,
              remaining?: int32
            }
          },
        },
        stripe?: {
          membership_type: "free" | "free_trial" | "pro" | "pro_plus" | "ultra" | "enterprise",
          payment_id?: string,
          days_remaining_on_trial: int32,
          subscription_status?: "trialing" | "active" | "incomplete" | "incomplete_expired" | "past_due" | "canceled" | "unpaid" | "paused",
          verified_student: bool,
          trial_eligible: bool,
          trial_length_days: int32,
          is_on_student_plan: bool,
          is_on_billable_auto: bool,
          customer_balance?: double,
          trial_was_cancelled: bool,
          is_team_member: bool,
          team_membership_type?: "free" | "free_trial" | "pro" | "pro_plus" | "ultra" | "enterprise",
          individual_membership_type?: "free" | "free_trial" | "pro" | "pro_plus" | "ultra" | "enterprise"
        },
        sessions?: [
          {
            session_id: string,
            type: "unspecified" | "web" | "client" | "bugbot" | "background_agent",
            created_at: string,
            expires_at: string
          }
        ]
      }
    ]
  ],
  tokens_count: uint64
}
```

#### Set Token Information

* Endpoint: `/tokens/set`
* Method: POST
* Authentication: Bearer Token
* Request Format:

```typescript
[
  [
    string,
    {
      primary_token: string,
      secondary_token?: string,
      checksum: {
        first: string,
        second: string,
      },
      client_key?: string,
      config_version?: string,
      session_id?: string,
      proxy?: string,
      timezone?: string,
      gcpp_host?: "Asia" | "EU" | "US",
      user?: {
        user_id: int32,
        email?: string,
        first_name?: string,
        last_name?: string,
        workos_id?: string,
        team_id?: number,
        created_at?: string,
        is_enterprise_user: bool,
        is_on_new_pricing: bool,
        privacy_mode_info: {
          privacy_mode: "unspecified" | "no_storage" | "no_training" | "usage_data_training_allowed" | "usage_codebase_training_allowed",
          is_enforced_by_team?: bool
        }
      },
      status: {
        enabled: bool
      },
      usage?: {
        billing_cycle_start: string,
        billing_cycle_end: string,
        membership_type: "free" | "free_trial" | "pro" | "pro_plus" | "ultra" | "enterprise",
        limit_type: "user" | "team",
        is_unlimited: bool,
        individual_usage: {
          plan?: {
            enabled: bool,
            used: int32,
            limit: int32,
            remaining: int32,
            breakdown: {
              included: int32,
              bonus: int32,
              total: int32
            }
          },
          on_demand?: {
            enabled: bool,
            used: int32,
            limit?: int32,
            remaining?: int32
          }
        },
        team_usage: {
          plan?: {
            enabled: bool,
            used: int32,
            limit: int32,
            remaining: int32,
            breakdown: {
              included: int32,
              bonus: int32,
              total: int32
            }
          },
          on_demand?: {
            enabled: bool,
            used: int32,
            limit?: int32,
            remaining?: int32
          }
        },
      },
      stripe?: {
        membership_type: "free" | "free_trial" | "pro" | "pro_plus" | "ultra" | "enterprise",
        payment_id?: string,
        days_remaining_on_trial: int32,
        subscription_status?: "trialing" | "active" | "incomplete" | "incomplete_expired" | "past_due" | "canceled" | "unpaid" | "paused",
        verified_student: bool,
        trial_eligible: bool,
        trial_length_days: int32,
        is_on_student_plan: bool,
        is_on_billable_auto: bool,
        customer_balance?: double,
        trial_was_cancelled: bool,
        is_team_member: bool,
        team_membership_type?: "free" | "free_trial" | "pro" | "pro_plus" | "ultra" | "enterprise",
        individual_membership_type?: "free" | "free_trial" | "pro" | "pro_plus" | "ultra" | "enterprise"
      }
    }
  ]
]
```

* Response Format:

```typescript
{
  status: "success",
  tokens_count: uint64,
  message: "Token files have been updated and reloaded"
}
```

#### Add Token

* Endpoint: `/tokens/add`
* Method: POST
* Authentication: Bearer Token
* Request Format:

```typescript
{
  tokens: [
    {
      alias?: string, // Optional, auto-generated if not provided
      token: string,
      checksum?: string, // Optional, auto-generated if not provided
      client_key?: string, // Optional, auto-generated if not provided
      session_id?: string, // Optional
      config_version?: string, // Optional
      proxy?: string, // Optional
      timezone?: string, // Optional
      gcpp_host?: string // Optional
    }
  ],
  enabled: bool
}
```

* Response Format:

```typescript
{
  status: "success",
  tokens_count: uint64,
  message: string  // "New tokens have been added and reloaded" or "No new tokens were added"
}
```

#### Delete Token

* Endpoint: `/tokens/del`
* Method: POST
* Authentication: Bearer Token
* Request Format:

```json
{
  "aliases": [string], // List of tokens to delete
  "include_failed_tokens": bool // Default is false
}
```

* Response Format:

```json
{
  "status": "success",
  "failed_tokens": [string] // Optional, returned based on include_failed_tokens, indicates tokens not found
}
```

* Expectation options:
  - simple: Returns only basic status
  - updated_tokens: Returns updated token list
  - failed_tokens: Returns list of tokens not found
  - detailed: Returns complete information (including updated_tokens and failed_tokens)

#### Set Token Tags (Deprecated)

* Endpoint: `/tokens/tags/set`
* Method: POST
* Authentication: Bearer Token
* Request Format:

```json
{
  "tokens": [string],
  "tags": {
    string: null | string // Key can be timezone: timezone identifier or proxy: proxy name
  }
}
```

* Response Format:

```json
{
  "status": "success",
  "message": string  // "Tags updated successfully"
}
```

#### Update Token Profile

* Endpoint: `/tokens/profile/update`
* Method: POST
* Authentication: Bearer Token
* Request Format:

```json
[
  string // aliases
]
```

* Response Format:

```json
{
  "status": "success",
  "message": "Updated {} token profiles, {} tokens failed to update"
}
```

#### Update Token Config Version

* Endpoint: `/tokens/config-version/update`
* Method: POST
* Authentication: Bearer Token
* Request Format:

```json
[
  string // aliases
]
```

* Response Format:

```json
{
  "status": "success",
  "message": "Updated {} token config versions, {} tokens failed to update"
}
```

#### Refresh Tokens

* Endpoint: `/tokens/refresh`
* Method: POST
* Authentication: Bearer Token
* Request Format:

```json
[
  string // aliases
]
```

* Response Format:

```json
{
  "status": "success",
  "message": "Refreshed {} tokens, {} tokens failed to refresh"
}
```

#### Set Token Status

* Endpoint: `/tokens/status/set`
* Method: POST
* Authentication: Bearer Token
* Request Format:

```typescript
{
  "aliases": [string],
  "enabled": bool
}
```

* Response Format:

```json
{
  "status": "success",
  "message": "Set status for {} tokens, {} tokens failed"
}
```

#### Set Token Alias

* Endpoint: `/tokens/alias/set`
* Method: POST
* Authentication: Bearer Token
* Request Format:

```json
{
  "{old_alias}": "{new_alias}"
}
```

* Response Format:

```json
{
  "status": "success",
  "message": "Set alias for {} tokens, {} tokens failed"
}
```

#### Set Token Proxy

* Endpoint: `/tokens/proxy/set`
* Method: POST
* Authentication: Bearer Token
* Request Format:

```json
{
  "aliases": [string],
  "proxy": string  // Optional, proxy address, null to clear proxy
}
```

* Response Format:

```json
{
  "status": "success",
  "message": "Set proxy for {} tokens, {} tokens failed"
}
```

#### Set Token Timezone

* Endpoint: `/tokens/timezone/set`
* Method: POST
* Authentication: Bearer Token
* Request Format:

```json
{
  "aliases": [string],
  "timezone": string  // Optional, timezone identifier (e.g., "Asia/Shanghai"), null to clear timezone
}
```

* Response Format:

```json
{
  "status": "success",
  "message": "Set timezone for {} tokens, {} tokens failed"
}
```

#### Merge Token Data

* Endpoint: `/tokens/merge`
* Method: POST
* Authentication: Bearer Token
* Request Format:

```json
{
  "{alias}": { // At least one of the following must exist, otherwise it will fail
    "primary_token": string, // Optional
    "secondary_token": string, // Optional
    "checksum": { // Optional
      "first": string,
      "second": string,
    },
    "client_key": string, // Optional
    "config_version": string, // Optional
    "session_id": string, // Optional
    "proxy": string, // Optional
    "timezone": string, // Optional
    "gcpp_host": object, // Optional
  }
}
```

* Response Format:

```json
{
  "status": "success",
  "message": "Merged {} tokens, {} tokens failed to merge"
}
```

#### Build API Key

* Endpoint: `/build-key`
* Method: POST
* Authentication: Bearer Token (required when SHARE_AUTH_TOKEN is enabled)
* Request Format:

```json
{
  "token": string,               // Format: JWT
  "checksum": {
    "first": string,             // Format: 64-character hex-encoded string
    "second": string,            // Format: 64-character hex-encoded string
  },
  "client_key": string,          // Format: 64-character hex-encoded string
  "config_version": string,      // Format: UUID
  "session_id": string,          // Format: UUID
  "proxy_name": string,          // Optional, specify proxy
  "timezone": string,            // Optional, specify timezone
  "gcpp_host": string,           // Optional, code completion region
  "disable_vision": bool,        // Optional, disable image processing capability
  "enable_slow_pool": bool,      // Optional, enable slow pool
  "include_web_references": bool,
  "usage_check_models": {        // Optional, usage check model configuration
    "type": "default" | "disabled" | "all" | "custom",
    "model_ids": string  // Effective when type is custom, comma-separated model ID list
  }
}
```

* Response Format:

```json
{
  "keys": [string]    // Returns generated key on success
}
```

Or on error:

```json
{
  "error": string  // Error message
}
```

Notes:

1. This endpoint is used to generate API Keys with dynamic configuration. It is an upgraded version of the direct token and checksum mode. Since version 0.3, the direct token and checksum method is no longer applicable.

2. The generated key format is: `sk-{encoded_config}`, where sk- is the default prefix (configurable)

3. usage_check_models configuration:
   - default: Use default model list (same as the default value of the `usage_check_models` field below)
   - disabled: Disable usage checking
   - all: Check all available models
   - custom: Use custom model list (specify in model_ids)

4. In the current version, the keys array always has length 3. The last 2 are cache-based and only work after the first one is used:
   1. Complete key (also exists in older versions)
   2. Base64-encoded version of the numeric key
   3. Plain text version of the numeric key

5. The numeric key consists of a 128-bit unsigned integer and a 64-bit unsigned integer, making it harder to crack than typical UUIDs.

### Proxy Management Endpoints

#### Get Proxy Configuration

* Endpoint: `/proxies/get`
* Method: POST
* Response Format:

```json
{
  "status": "success",
  "proxies": {
    "proxies": {
      "proxy_name": "non" | "sys" | "http://proxy-url",
    },
    "general": string // Currently used general proxy name
  },
  "proxies_count": number,
  "general_proxy": string,
  "message": string // Optional
}
```

#### Set Proxy Configuration

* Endpoint: `/proxies/set`
* Method: POST
* Request Format:

```json
{
  "proxies": {
    "{proxy_name}": "non" | "sys" | "http://proxy-url"
  },
  "general": string  // General proxy name to set
}
```

* Response Format:

```json
{
  "status": "success",
  "proxies_count": number,
  "message": "Proxy configuration updated"
}
```

#### Add Proxy

* Endpoint: `/proxies/add`
* Method: POST
* Request Format:

```json
{
  "proxies": {
    "{proxy_name}": "non" | "sys" | "http://proxy-url"
  }
}
```

* Response Format:

```json
{
  "status": "success",
  "proxies_count": number,
  "message": string  // "Added X new proxies" or "No new proxies added"
}
```

#### Delete Proxy

* Endpoint: `/proxies/del`
* Method: POST
* Request Format:

```json
{
  "names": [string],  // List of proxy names to delete
  "expectation": "simple" | "updated_proxies" | "failed_names" | "detailed"  // Default is simple
}
```

* Response Format:

```json
{
  "status": "success",
  "updated_proxies": {  // Optional, returned based on expectation
    "proxies": {
      "proxy_name": "non" | "sys" | "http://proxy-url"
    },
    "general": string
  },
  "failed_names": [string]  // Optional, returned based on expectation, indicates proxy names not found
}
```

#### Set General Proxy

* Endpoint: `/proxies/set-general`
* Method: POST
* Request Format:

```json
{
  "name": string  // Proxy name to set as general proxy
}
```

* Response Format:

```json
{
  "status": "success",
  "message": "General proxy has been set"
}
```

#### Proxy Type Description

* `non`: No proxy
* `sys`: Use system proxy
* Other: Specific proxy URL address (e.g., `http://proxy-url`)

#### Notes

1. Proxy names must be unique. Adding proxies with duplicate names will be ignored.
2. When setting the general proxy, the specified proxy name must exist in the current proxy configuration.
3. Expectation parameter description for deleting proxies:
   - simple: Returns only basic status
   - updated_proxies: Returns updated proxy configuration
   - failed_names: Returns list of proxy names not found
   - detailed: Returns complete information (including updated_proxies and failed_names)

### Error Format

All endpoints return a unified error format when an error occurs:

```json
{
  "status": "error",
  "code": number,   // Optional
  "error": string,  // Optional, error details
  "message": string // Error message
}
```

### Configuration Management Endpoints

#### Get Configuration

* Endpoint: `/config/get`
* Method: POST
* Authentication: Bearer Token
* Request Format: None
* Response Format: `x-config-hash` + text

#### Update Configuration

* Endpoint: `/config/set`
* Method: POST
* Authentication: Bearer Token
* Request Format: `x-config-hash` + text
* Response Format: 204 indicates changed, 200 indicates unchanged, others are errors

#### Reload Configuration

* Endpoint: `/config/reload`
* Method: GET
* Authentication: Bearer Token
* Request Format: `x-config-hash`
* Response Format: 204 indicates changed, 200 indicates unchanged, others are errors

### Log Management Endpoints

#### Get Logs

* Endpoint: `/logs`
* Method: GET
* Response Format: Returns different content types based on configuration (default, text, or HTML)

#### Get Log Data

* Endpoint: `/logs/get`
* Method: POST
* Authentication: Bearer Token
* Request Format:

```typescript
{
  "query": {
    // Pagination and sorting control
    "limit": number,            // Optional, limit number of records returned
    "offset": number,           // Optional, starting position offset
    "reverse": bool,            // Optional, reverse order, default false (old to new), true for new to old

    // Time range filtering
    "from_date": string,        // Optional, start datetime, RFC3339 format
    "to_date": string,          // Optional, end datetime, RFC3339 format

    // User identification filtering
    "user_id": string,          // Optional, exact match by user ID
    "email": string,            // Optional, filter by user email (supports partial match)
    "membership_type": string,  // Optional, filter by membership type ("free"/"free_trial"/"pro"/"pro_plus"/"ultra"/"enterprise")

    // Core business filtering
    "status": string,           // Optional, filter by status ("pending"/"success"/"failure")
    "model": string,            // Optional, filter by model name (supports partial match)
    "include_models": [string], // Optional, include specific models
    "exclude_models": [string], // Optional, exclude specific models

    // Request characteristic filtering
    "stream": bool,             // Optional, whether it's a streaming request
    "has_chain": bool,          // Optional, whether it contains a conversation chain
    "has_usage": bool,          // Optional, whether it has usage information

    // Error-related filtering
    "has_error": bool,          // Optional, whether it contains errors
    "error": string,            // Optional, filter by error (supports partial match)

    // Performance metric filtering
    "min_total_time": number,   // Optional, minimum total time (seconds)
    "max_total_time": number,   // Optional, maximum total time (seconds)
    "min_tokens": number,       // Optional, minimum token count (input + output)
    "max_tokens": number        // Optional, maximum token count
  }
}
```

* Response Format:

```typescript
{
  status: "success",
  total: uint64,
  active?: uint64,
  error?: uint64,
  logs: [
    {
      id: uint64,
      timestamp: string,
      model: string,
      token_info: {
        key: string,
        usage?: {
          billing_cycle_start: string,
          billing_cycle_end: string,
          membership_type: "free" | "free_trial" | "pro" | "pro_plus" | "ultra" | "enterprise",
          limit_type: "user" | "team",
          is_unlimited: bool,
          individual_usage: {
            plan?: {
              enabled: bool,
              used: int32,
              limit: int32,
              remaining: int32,
              breakdown: {
                included: int32,
                bonus: int32,
                total: int32
              }
            },
            on_demand?: {
              enabled: bool,
              used: int32,
              limit?: int32,
              remaining?: int32
            }
          },
          team_usage: {
            plan?: {
              enabled: bool,
              used: int32,
              limit: int32,
              remaining: int32,
              breakdown: {
                included: int32,
                bonus: int32,
                total: int32
              }
            },
            on_demand?: {
              enabled: bool,
              used: int32,
              limit?: int32,
              remaining?: int32
            }
          },
        },
        stripe?: {
          membership_type: "free" | "free_trial" | "pro" | "pro_plus" | "ultra" | "enterprise",
          payment_id?: string,
          days_remaining_on_trial: int32,
          subscription_status?: "trialing" | "active" | "incomplete" | "incomplete_expired" | "past_due" | "canceled" | "unpaid" | "paused",
          verified_student: bool,
          trial_eligible: bool,
          trial_length_days: int32,
          is_on_student_plan: bool,
          is_on_billable_auto: bool,
          customer_balance?: double,
          trial_was_cancelled: bool,
          is_team_member: bool,
          team_membership_type?: "free" | "free_trial" | "pro" | "pro_plus" | "ultra" | "enterprise",
          individual_membership_type?: "free" | "free_trial" | "pro" | "pro_plus" | "ultra" | "enterprise"
        }
      },
      chain: {
        delays?: [
          string,
          [
            number, // chars count
            number // time
          ]
        ],
        usage?: {
          input: int32,
          output: int32,
          cache_write: int32,
          cache_read: int32,
          cents: float
        }
      },
      timing: {
        total: double
      },
      stream: bool,
      status: "pending" | "success" | "failure",
      error?: string | {
        error:string,
        details:string
      }
    }
  ],
  timestamp: string
}
```

* Notes:
  - All query parameters are optional
  - Administrators can view all logs, regular users can only view logs related to their tokens
  - If an invalid status or membership type is provided, empty results will be returned
  - Datetime format must follow RFC3339 standard, e.g., "2024-03-20T15:30:00+08:00"
  - Email and model name support partial matching

#### Get Log Tokens

* Endpoint: `/logs/tokens/get`
* Method: POST
* Authentication: Bearer Token
* Request Format:

```typescript
[
  string
]
```

* Response Format:

```typescript
{
  status: "success",
  tokens: {
    {key}: {
      primary_token: string,
      secondary_token?: string,
      checksum: {
        first: string,
        second: string,
      },
      client_key?: string,
      config_version?: string,
      session_id?: string,
      proxy?: string,
      timezone?: string,
      gcpp_host?: "Asia" | "EU" | "US",
      user?: {
        user_id: int32,
        email?: string,
        first_name?: string,
        last_name?: string,
        workos_id?: string,
        team_id?: number,
        created_at?: string,
        is_enterprise_user: bool,
        is_on_new_pricing: bool,
        privacy_mode_info: {
          privacy_mode: "unspecified" | "no_storage" | "no_training" | "usage_data_training_allowed" | "usage_codebase_training_allowed",
          is_enforced_by_team?: bool
        }
      }
    }
  },
  total: uint64,
  timestamp: string
}
```

### Static Resource Endpoints

#### Environment Variable Example

* Endpoint: `/env-example`
* Method: GET
* Response Format: Text

#### Configuration File Example

* Endpoint: `/config-example`
* Method: GET
* Response Format: Text

#### Documentation

* Endpoint: `/readme`
* Method: GET
* Response Format: HTML

#### License

* Endpoint: `/license`
* Method: GET
* Response Format: HTML

### Health Check Endpoint

* Endpoint: `/health`
* Method: GET
* Authentication: Not required
* Response Format: Returns different content types based on configuration (default JSON, text, or HTML)

#### Response Structure

```json
{
  "status": "success",
  "service": {
    "name": "cursor-api",
    "version": "1.0.0",
    "is_debug": false,
    "build": {
      "version": 1,
      "timestamp": "2024-01-15T10:30:00Z",
      "is_debug": false,
      "is_prerelease": false
    }
  },
  "runtime": {
    "started_at": "2024-01-15T10:00:00+08:00",
    "uptime_seconds": 1800,
    "requests": {
      "total": 1250,
      "active": 3,
      "errors": 12
    }
  },
  "system": {
    "memory": {
      "used_bytes": 134217728,
      "used_percentage": 12.5,
      "available_bytes": 1073741824
    },
    "cpu": {
      "usage_percentage": 15.2,
      "load_average": [0.8, 1.2, 1.5]
    }
  },
  "capabilities": {
    "models": ["gpt-4", "claude-3"],
    "endpoints": ["/v1/chat/completions", "/v1/messages"],
    "features": [".."]
  }
}
```

#### Field Description

| Field                           | Type   | Description                                                 |
|---------------------------------|--------|-------------------------------------------------------------|
| `status`                        | string | Service status: "success", "warning", "error"               |
| `service.name`                  | string | Service name                                                |
| `service.version`               | string | Service version                                             |
| `service.is_debug`              | bool   | Whether in debug mode                                       |
| `service.build.version`         | number | Build version number (only when preview feature is enabled) |
| `service.build.timestamp`       | string | Build timestamp                                             |
| `service.build.is_prerelease`   | bool   | Whether it's a prerelease version                           |
| `runtime.started_at`            | string | Service start time                                          |
| `runtime.uptime_seconds`        | number | Uptime (seconds)                                            |
| `runtime.requests.total`        | number | Total requests                                              |
| `runtime.requests.active`       | number | Current active requests                                     |
| `runtime.requests.errors`       | number | Error requests                                              |
| `system.memory.used_bytes`      | number | Used memory (bytes)                                         |
| `system.memory.used_percentage` | number | Memory usage (%)                                            |
| `system.memory.available_bytes` | number | Available memory (bytes, optional)                          |
| `system.cpu.usage_percentage`   | number | CPU usage (%)                                               |
| `system.cpu.load_average`       | array  | System load [1min, 5min, 15min]                             |
| `capabilities.models`           | array  | Supported model list                                        |
| `capabilities.endpoints`        | array  | Available API endpoints                                     |
| `capabilities.features`         | array  | Supported features                                          |

### Other Endpoints

#### Generate Random UUID

* Endpoint: `/gen-uuid`
* Method: GET
* Response Format:

```plaintext
string
```

#### Generate Random Hash

* Endpoint: `/gen-hash`
* Method: GET
* Response Format:

```plaintext
string
```

#### Generate Random Checksum

* Endpoint: `/gen-checksum`
* Method: GET
* Response Format:

```plaintext
string
```

#### Generate Random Token (Deprecated)

* Endpoint: `/gen-token`
* Method: GET
* Response Format:

```plaintext
string
```

#### Get Current Checksum Header

* Endpoint: `/get-checksum-header`
* Method: GET
* Response Format:

```plaintext
string
```

#### Get Account Information

* Endpoint: `/token-profile/get`
* Method: POST
* Authentication: Bearer Token
* Request Format:

```typescript
{
  session_token: string,
  web_token: string,
  proxy_name?: string,
  include_sessions: bool
}
```

* Response Format:

```typescript
{
  token_profile: [
    null | {
      billing_cycle_start: string,
      billing_cycle_end: string,
      membership_type: "free" | "free_trial" | "pro" | "pro_plus" | "ultra" | "enterprise",
      limit_type: "user" | "team",
      is_unlimited: bool,
      individual_usage: {
        plan?: {
          enabled: bool,
          used: int32,
          limit: int32,
          remaining: int32,
          breakdown: {
            included: int32,
            bonus: int32,
            total: int32
          }
        },
        on_demand?: {
          enabled: bool,
          used: int32,
          limit?: int32,
          remaining?: int32
        }
      },
      team_usage: {
        plan?: {
          enabled: bool,
          used: int32,
          limit: int32,
          remaining: int32,
          breakdown: {
            included: int32,
            bonus: int32,
            total: int32
          }
        },
        on_demand?: {
          enabled: bool,
          used: int32,
          limit?: int32,
          remaining?: int32
        }
      },
    },
    null | {
      membership_type: "free" | "free_trial" | "pro" | "pro_plus" | "ultra" | "enterprise",
      payment_id?: string,
      days_remaining_on_trial: int32,
      subscription_status?: "trialing" | "active" | "incomplete" | "incomplete_expired" | "past_due" | "canceled" | "unpaid" | "paused",
      verified_student: bool,
      trial_eligible: bool,
      trial_length_days: int32,
      is_on_student_plan: bool,
      is_on_billable_auto: bool,
      customer_balance?: double,
      trial_was_cancelled: bool,
      is_team_member: bool,
      team_membership_type?: "free" | "free_trial" | "pro" | "pro_plus" | "ultra" | "enterprise",
      individual_membership_type?: "free" | "free_trial" | "pro" | "pro_plus" | "ultra" | "enterprise"
    },
    null | {
      user_id: int32,
      email?: string,
      first_name?: string,
      last_name?: string,
      workos_id?: string,
      team_id?: number,
      created_at?: string,
      is_enterprise_user: bool,
      is_on_new_pricing: bool,
      privacy_mode_info: {
        privacy_mode: "unspecified" | "no_storage" | "no_training" | "usage_data_training_allowed" | "usage_codebase_training_allowed",
        is_enforced_by_team?: bool
      }
    },
    null | [
      {
        session_id: string,
        type: "unspecified" | "web" | "client" | "bugbot" | "background_agent",
        created_at: string,
        expires_at: string
      }
    ]
  ]
}
```

If an error occurs, the response format is:

```json
{
  "error": string
}
```

#### Get Config Version

* Endpoint: `/config-version/get`
* Method: POST
* Authentication: Bearer Token (required when SHARE_AUTH_TOKEN is enabled)
* Request Format:

```json
{
  "token": string,               // Format: JWT
  "checksum": {
    "first": string,             // Format: 64-character hex-encoded string
    "second": string,            // Format: 64-character hex-encoded string
  },
  "client_key": string,          // Format: 64-character hex-encoded string
  "session_id": string,          // Format: UUID
  "proxy_name": string,          // Optional, specify proxy
  "timezone": string,            // Optional, specify timezone
  "gcpp_host": string            // Optional, code completion region
}
```

* Response Format:

```json
{
  "config_version": string    // Returns generated UUID on success
}
```

Or on error:

```json
{
  "error": string  // Error message
}
```

#### Get Upgrade Token (Deprecated)

* Endpoint: `/token-upgrade`
* Method: POST
* Authentication: Token included in request body
* Request Format:

```json
{
  "token": string
}
```

* Response Format:

```json
{
  "status": "success" | "failure" | "error",
  "message": string,
  "result": string // optional
}
```

## Copilot++ API Documentation

1. All related endpoints require `x-client-key`. See `/gen-hash` for format (32 bytes).
2. Cookie `FilesyncCookie` is 16 bytes and remains unchanged as long as the workspace doesn't change.
3. Cookies like `AWSALBAPP-0` have a 7-day validity period and may change. See Amazon documentation for details.
4. `FilesyncCookie` and `AWSALBAPP` are always set by `/file/upload` or `/file/sync`.
5. All endpoints below use POST method and require authentication.

### Get Code Completion Service Configuration

* Endpoint: `/cpp/config`

#### Request Format

```json
{
  "is_nightly": bool,  // Optional, whether to use nightly version
  "model": string,     // Model name
  "supports_cpt": bool // Optional, whether CPT is supported
}
```

### Response Format

```json
{
  "above_radius": number,                                        // Optional, scan radius above
  "below_radius": number,                                        // Optional, scan radius below
  "merge_behavior": {                                            // Optional, merge behavior
    "type": string,
    "limit": number,                                             // Optional, limit
    "radius": number                                             // Optional, radius
  },
  "is_on": bool,                                                 // Optional, whether enabled
  "is_ghost_text": bool,                                         // Optional, whether to use ghost text
  "should_let_user_enable_cpp_even_if_not_pro": bool,            // Optional, allow non-pro users to enable
  "heuristics": [                                                // Enabled heuristic rules list
    "lots_of_added_text",
    "duplicating_line_after_suggestion",
    "duplicating_multiple_lines_after_suggestion",
    "reverting_user_change",
    "output_extends_beyond_range_and_is_repeated",
    "suggesting_recently_rejected_edit"
  ],
  "exclude_recently_viewed_files_patterns": [string],            // Recently viewed files exclusion patterns
  "enable_rvf_tracking": bool,                                   // Whether to enable RVF tracking
  "global_debounce_duration_millis": number,                     // Global debounce duration (milliseconds)
  "client_debounce_duration_millis": number,                     // Client debounce duration (milliseconds)
  "cpp_url": string,                                             // CPP service URL
  "use_whitespace_diff_history": bool,                           // Whether to use whitespace diff history
  "import_prediction_config": {                                  // Import prediction configuration
    "is_disabled_by_backend": bool,                              // Whether disabled by backend
    "should_turn_on_automatically": bool,                        // Whether to turn on automatically
    "python_enabled": bool                                       // Whether Python is enabled
  },
  "enable_filesync_debounce_skipping": bool,                     // Whether to enable filesync debounce skipping
  "check_filesync_hash_percent": number,                         // Filesync hash check percentage
  "geo_cpp_backend_url": string,                                 // Geographic CPP backend URL
  "recently_rejected_edit_thresholds": {                         // Optional, recently rejected edit thresholds
    "hard_reject_threshold": number,                             // Hard reject threshold
    "soft_reject_threshold": number                              // Soft reject threshold
  },
  "is_fused_cursor_prediction_model": bool,                      // Whether to use fused cursor prediction model
  "include_unchanged_lines": bool,                               // Whether to include unchanged lines
  "should_fetch_rvf_text": bool,                                 // Whether to fetch RVF text
  "max_number_of_cleared_suggestions_since_last_accept": number, // Optional, max cleared suggestions since last accept
  "suggestion_hint_config": {                                    // Optional, suggestion hint configuration
    "important_lsp_extensions": [string],                        // Important LSP extensions
    "enabled_for_path_extensions": [string]                      // Enabled path extensions
  }
}
```

### Get Available Code Completion Models

* Endpoint: `/cpp/models`

#### Request Format

None

### Response Format

```json
{
  "models": [string],     // Available model list
  "default_model": string // Optional, default model
}
```

### Upload File

* Endpoint: `/file/upload`

#### Request Format

```json
{
  "uuid": string,                    // File identifier
  "relative_workspace_path": string, // File path relative to workspace
  "contents": string,                // File contents
  "model_version": number,           // Model version
  "sha256_hash": string              // Optional, SHA256 hash of file
}
```

### Response Format

```json
{
  "error": string // Error type: unspecified, non_existant, hash_mismatch
}
```

### Sync File Changes

* Endpoint: `/file/sync`

#### Request Format

```json
{
  "uuid": string,                                // File identifier
  "relative_workspace_path": string,             // File path relative to workspace
  "model_version": number,                       // Model version
  "filesync_updates": [                          // File sync update array
    {
      "model_version": number,                   // Model version
      "relative_workspace_path": string,         // File path relative to workspace
      "updates": [                               // Single update request array
        {
          "start_position": number,              // Update start position
          "end_position": number,                // Update end position
          "change_length": number,               // Change length
          "replaced_string": string,             // Replaced string
          "range": {                             // Simple range
            "start_line_number": number,         // Start line number
            "start_column": number,              // Start column
            "end_line_number_inclusive": number, // End line number (inclusive)
            "end_column": number                 // End column
          }
        }
      ],
      "expected_file_length": number             // Expected file length
    }
  ],
  "sha256_hash": string                          // SHA256 hash of file
}
```

### Response Format

```json
{
  "error": string // Error type: unspecified, non_existant, hash_mismatch
}
```

### Streaming Code Completion

* Endpoint: `/cpp/stream`

#### Request Format

```typescript
{
  current_file: {                                                 // Current file information
    relative_workspace_path: string,                              // File path relative to workspace
    contents: string,                                             // File contents
    rely_on_filesync: bool,                                       // Whether to rely on file sync
    sha_256_hash?: string,                                        // Optional, SHA256 hash of file contents
    top_chunks: [                                                 // Top code chunks from BM25 retrieval
      {
        content: string,                                          // Code chunk content
        range: {                                                  // SimplestRange simplest range
          start_line: int32,                                      // Start line number
          end_line_inclusive: int32                               // End line number (inclusive)
        },
        score: int32,                                             // BM25 score
        relative_path: string                                     // Relative path of file containing code chunk
      }
    ],
    contents_start_at_line: int32,                                // Content start line number (usually 0)
    cursor_position: {                                            // CursorPosition cursor position
      line: int32,                                                // Line number (0-based)
      column: int32                                               // Column number (0-based)
    },
    dataframes: [                                                 // DataframeInfo dataframe info (for data analysis scenarios)
      {
        name: string,                                             // Dataframe variable name
        shape: string,                                            // Shape description, e.g. "(100, 5)"
        data_dimensionality: int32,                               // Data dimensionality
        columns: [                                                // Column definitions
          {
            key: string,                                          // Column name
            type: string                                          // Column data type
          }
        ],
        row_count: int32,                                         // Row count
        index_column: string                                      // Index column name
      }
    ],
    total_number_of_lines: int32,                                 // Total number of lines in file
    language_id: string,                                          // Language identifier (e.g. "python", "rust")
    selection?: {                                                 // Optional, CursorRange current selection range
      start_position: {                                           // CursorPosition start position
        line: int32,                                              // Line number
        column: int32                                             // Column number
      },
      end_position: {                                             // CursorPosition end position
        line: int32,                                              // Line number
        column: int32                                             // Column number
      }
    },
    alternative_version_id?: int32,                               // Optional, alternative version ID
    diagnostics: [                                                // Diagnostic diagnostics array
      {
        message: string,                                          // Diagnostic message content
        range: {                                                  // CursorRange diagnostic range
          start_position: {                                       // CursorPosition start position
            line: int32,                                          // Line number
            column: int32                                         // Column number
          },
          end_position: {                                         // CursorPosition end position
            line: int32,                                          // Line number
            column: int32                                         // Column number
          }
        },
        severity: "error" | "warning" | "information" | "hint",   // DiagnosticSeverity severity level
        related_information: [                                    // RelatedInformation related information
          {
            message: string,                                      // Related information message
            range: {                                              // CursorRange related information range
              start_position: {                                   // CursorPosition start position
                line: int32,                                      // Line number
                column: int32                                     // Column number
              },
              end_position: {                                     // CursorPosition end position
                line: int32,                                      // Line number
                column: int32                                     // Column number
              }
            }
          }
        ]
      }
    ],
    file_version?: int32,                                         // Optional, file version number (for incremental updates)
    workspace_root_path: string,                                  // Workspace root path (absolute path)
    line_ending?: string,                                         // Optional, line ending ("\n" or "\r\n")
    file_git_context: {                                           // FileGit Git context information
      commits: [                                                  // GitCommit related commits array
        {
          commit: string,                                         // Commit hash
          author: string,                                         // Author
          date: string,                                           // Commit date
          message: string                                         // Commit message
        }
      ]
    }
  },
  diff_history: [string],                                         // Diff history (deprecated, use file_diff_histories instead)
  model_name?: string,                                            // Optional, specify model name to use
  linter_errors?: {                                               // Optional, LinterErrors linter error info
    relative_workspace_path: string,                              // Relative path of file with errors
    errors: [                                                     // LinterError error array
      {
        message: string,                                          // Error message
        range: {                                                  // CursorRange error range
          start_position: {                                       // CursorPosition start position
            line: int32,                                          // Line number
            column: int32                                         // Column number
          },
          end_position: {                                         // CursorPosition end position
            line: int32,                                          // Line number
            column: int32                                         // Column number
          }
        },
        source?: string,                                          // Optional, error source (e.g. "eslint", "pyright")
        related_information: [                                    // Diagnostic.RelatedInformation related info
          {
            message: string,                                      // Related info message
            range: {                                              // CursorRange related info range
              start_position: {                                   // CursorPosition start position
                line: int32,                                      // Line number
                column: int32                                     // Column number
              },
              end_position: {                                     // CursorPosition end position
                line: int32,                                      // Line number
                column: int32                                     // Column number
              }
            }
          }
        ],
        severity?: "error" | "warning" | "information" | "hint"   // Optional, DiagnosticSeverity severity level
      }
    ],
    file_contents: string                                         // File contents (for error context)
  },
  context_items: [                                                // CppContextItem context items array
    {
      contents: string,                                           // Context content
      symbol?: string,                                            // Optional, symbol name
      relative_workspace_path: string,                            // Relative path of context file
      score: float                                                // Relevance score
    }
  ],
  diff_history_keys: [string],                                    // Diff history keys (deprecated)
  give_debug_output?: bool,                                       // Optional, whether to output debug info
  file_diff_histories: [                                          // CppFileDiffHistory file diff history array
    {
      file_name: string,                                          // File name
      diff_history: [string],                                     // Diff history array, format: "lineNum-|oldContent\nlineNum+|newContent\n"
      diff_history_timestamps: [double]                           // Diff timestamps array (Unix millisecond timestamps)
    }
  ],
  merged_diff_histories: [                                        // CppFileDiffHistory merged diff history
    {
      file_name: string,                                          // File name
      diff_history: [string],                                     // Merged diff history
      diff_history_timestamps: [double]                           // Timestamps array
    }
  ],
  block_diff_patches: [                                           // BlockDiffPatch block-level diff patches
    {
      start_model_window: {                                       // ModelWindow model window start state
        lines: [string],                                          // Code lines in window
        start_line_number: int32,                                 // Window start line number
        end_line_number: int32                                    // Window end line number
      },
      changes: [                                                  // Change changes array
        {
          text: string,                                           // Changed text
          range: {                                                // IRange change range
            start_line_number: int32,                             // Start line number
            start_column: int32,                                  // Start column number
            end_line_number: int32,                               // End line number
            end_column: int32                                     // End column number
          }
        }
      ],
      relative_path: string,                                      // File relative path
      model_uuid: string,                                         // Model UUID (for tracking completion source)
      start_from_change_index: int32                              // Start applying from which change index
    }
  ],
  is_nightly?: bool,                                              // Optional, whether nightly build version
  is_debug?: bool,                                                // Optional, whether debug mode
  immediately_ack?: bool,                                         // Optional, whether to immediately acknowledge request
  enable_more_context?: bool,                                     // Optional, whether to enable more context retrieval
  parameter_hints: [                                              // CppParameterHint parameter hints array
    {
      label: string,                                              // Parameter label (e.g. "x: int")
      documentation?: string                                      // Optional, parameter documentation
    }
  ],
  lsp_contexts: [                                                 // LspSubgraphFullContext LSP subgraph context
    {
      uri?: string,                                               // Optional, file URI
      symbol_name: string,                                        // Symbol name
      positions: [                                                // LspSubgraphPosition positions array
        {
          line: int32,                                            // Line number
          character: int32                                        // Character position
        }
      ],
      context_items: [                                            // LspSubgraphContextItem context items
        {
          uri?: string,                                           // Optional, URI
          type: string,                                           // Type (e.g. "definition", "reference")
          content: string,                                        // Content
          range?: {                                               // Optional, LspSubgraphRange range
            start_line: int32,                                    // Start line
            start_character: int32,                               // Start character
            end_line: int32,                                      // End line
            end_character: int32                                  // End character
          }
        }
      ],
      score: float                                                // Relevance score
    }
  ],
  cpp_intent_info?: {                                             // Optional, CppIntentInfo code completion intent info
    source: "line_change" | "typing" | "option_hold" |            // Trigger source
            "linter_errors" | "parameter_hints" | 
            "cursor_prediction" | "manual_trigger" | 
            "editor_change" | "lsp_suggestions"
  },
  workspace_id?: string,                                          // Optional, workspace unique identifier
  additional_files: [                                             // AdditionalFile additional files array
    {
      relative_workspace_path: string,                            // File relative path
      is_open: bool,                                              // Whether open in editor
      visible_range_content: [string],                            // Visible range content (by line)
      last_viewed_at?: double,                                    // Optional, last viewed time (Unix millisecond timestamp)
      start_line_number_one_indexed: [int32],                     // Visible range start line number (1-based index)
      visible_ranges: [                                           // LineRange visible ranges array
        {
          start_line_number: int32,                               // Start line number
          end_line_number_inclusive: int32                        // End line number (inclusive)
        }
      ]
    }
  ],
  control_token?: "quiet" | "loud" | "op",                        // Optional, ControlToken control token
  client_time?: double,                                           // Optional, client time (Unix millisecond timestamp)
  filesync_updates: [                                             // FilesyncUpdateWithModelVersion file sync incremental updates
    {
      model_version: int32,                                       // Model version number
      relative_workspace_path: string,                            // File relative path
      updates: [                                                  // SingleUpdateRequest update operations array
        {
          start_position: int32,                                  // Start position (character offset, 0-based)
          end_position: int32,                                    // End position (character offset, 0-based)
          change_length: int32,                                   // Length after change
          replaced_string: string,                                // Replaced string content
          range: {                                                // SimpleRange change range
            start_line_number: int32,                             // Start line number
            start_column: int32,                                  // Start column number
            end_line_number_inclusive: int32,                     // End line number (inclusive)
            end_column: int32                                     // End column number
          }
        }
      ],
      expected_file_length: int32                                 // Expected file length after applying updates
    }
  ],
  time_since_request_start: double,                               // Time since request start (milliseconds)
  time_at_request_send: double,                                   // Timestamp when request was sent (Unix millisecond timestamp)
  client_timezone_offset?: double,                                // Optional, client timezone offset (minutes, e.g. -480 for UTC+8)
  lsp_suggested_items?: {                                         // Optional, LspSuggestedItems LSP suggested items
    suggestions: [                                                // LspSuggestion suggestions array
      {
        label: string                                             // Suggestion label
      }
    ]
  },
  supports_cpt?: bool,                                            // Optional, whether supports CPT (Code Patch Token) format
  supports_crlf_cpt?: bool,                                       // Optional, whether supports CRLF line ending CPT format
  code_results: [                                                 // CodeResult code retrieval results
    {
      code_block: {                                               // CodeBlock code block
        relative_workspace_path: string,                          // File relative path
        file_contents?: string,                                   // Optional, full file contents
        file_contents_length?: int32,                             // Optional, file contents length
        range: {                                                  // CursorRange code block range
          start_position: {                                       // CursorPosition start position
            line: int32,                                          // Line number
            column: int32                                         // Column number
          },
          end_position: {                                         // CursorPosition end position
            line: int32,                                          // Line number
            column: int32                                         // Column number
          }
        },
        contents: string,                                         // Code block contents
        signatures: {                                             // Signatures signature info
          ranges: [                                               // CursorRange signature ranges array
            {
              start_position: {                                   // CursorPosition start position
                line: int32,                                      // Line number
                column: int32                                     // Column number
              },
              end_position: {                                     // CursorPosition end position
                line: int32,                                      // Line number
                column: int32                                     // Column number
              }
            }
          ]
        },
        override_contents?: string,                               // Optional, override contents
        original_contents?: string,                               // Optional, original contents
        detailed_lines: [                                         // DetailedLine detailed line info
          {
            text: string,                                         // Line text
            line_number: float,                                   // Line number (float to support virtual lines)
            is_signature: bool                                    // Whether signature line
          }
        ],
        file_git_context: {                                       // FileGit Git context
          commits: [                                              // GitCommit commits array
            {
              commit: string,                                     // Commit hash
              author: string,                                     // Author
              date: string,                                       // Commit date
              message: string                                     // Commit message
            }
          ]
        }
      },
      score: float                                                // Retrieval relevance score
    }
  ]
}
```

### Response Format (SSE Stream)

The server returns streaming responses via Server-Sent Events (SSE). Each event contains a `type` field to distinguish message types.

---

#### Event Types

**1. model_info** - Model Information
```typescript
{
  type: "model_info",
  is_fused_cursor_prediction_model: bool,
  is_multidiff_model: bool
}
```

---

**2. range_replace** - Range Replacement
```typescript
{
  type: "range_replace",
  start_line_number: int32,                  // Start line (1-based)
  end_line_number_inclusive: int32,          // End line (1-based, inclusive)
  binding_id?: string,
  should_remove_leading_eol?: bool
}
```
> **Note**: The replacement text content is sent via subsequent `text` events

---

**3. text** - Text Content
```typescript
{
  type: "text",
  text: string
}
```
> **Description**: Main content of streaming output, client should accumulate

---

**4. cursor_prediction** - Cursor Prediction
```typescript
{
  type: "cursor_prediction",
  relative_path: string,
  line_number_one_indexed: int32,
  expected_content: string,
  should_retrigger_cpp: bool,
  binding_id?: string
}
```

---

**5. done_edit** - Edit Complete
```typescript
{
  type: "done_edit"
}
```

---

**6. begin_edit** - Edit Start
```typescript
{
  type: "begin_edit"
}
```

---

**7. done_stream** - Content Phase End
```typescript
{
  type: "done_stream"
}
```
> **Description**: May be followed by `debug` messages

---

**8. debug** - Debug Information
```typescript
{
  type: "debug",
  model_input?: string,
  model_output?: string,
  stream_time?: string,
  total_time?: string,
  ttft_time?: string,
  server_timing?: string
}
```
> **Description**: May appear multiple times, frontend can accumulate for statistics

---

**9. error** - Error
```typescript
{
  type: "error",
  error: {
    code: uint16,                            // Non-zero error code
    type: string,                            // Error type
    details?: {                              // Optional detailed information
      title: string,
      detail: string,
      additional_info?: Record<string, string>
    }
  }
}
```

---

**10. stream_end** - Stream End
```typescript
{
  type: "stream_end"
}
```

---

#### Typical Message Sequences

**Basic Scenario:**
```
model_info
range_replace        // Specify range
text (xN)           // Streaming text
done_edit
done_stream
debug (xN)          // Optional multiple debug messages
stream_end
```

**Multiple Edits:**
```
model_info
range_replace
text (xN)
done_edit
begin_edit          // Next edit
range_replace
text (xN)
cursor_prediction   // Optional
done_edit
done_stream
stream_end
```

---

#### Client Processing Guidelines

1. **Accumulate Text**
   - `range_replace` specifies the range
   - Accumulate all subsequent `text` content
   - Apply changes when `done_edit` is received

2. **Newline Handling**
   - Remove the first newline when `should_remove_leading_eol=true`

3. **Multiple Edit Sessions**
   - `begin_edit` marks the start of a new session
   - `binding_id` is used to associate multiple edits from the same completion

4. **Error Handling**
   - When `error` appears in the stream, the client should abort the current operation

5. **Debug Information**
   - Multiple `debug` messages may appear after `done_stream`
   - Frontend can accumulate for performance analysis

## Acknowledgments

Thanks to the following projects and contributors:

- [cursor-api](https://github.com/wisdgod/cursor-api) - This project itself
- [zhx47/cursor-api](https://github.com/zhx47/cursor-api) - Provided the main reference during the initial development of this project
- [luolazyandlazy/cursorToApi](https://github.com/luolazyandlazy/cursorToApi) - zhx47/cursor-api was optimized based on this project

## About Sponsorship

Thank you for my continuous updates over 8+ months and everyone's support! If you want to sponsor, please contact me directly. I generally won't refuse.

Someone mentioned adding a QR code, but let's skip that. If you find it useful, feel free to show some support. It's no big deal. I'll do what I can when I have time, but it does take a lot of mental energy.

~~How about sending a red packet to my email?~~

**Sponsorship should only be given if you genuinely want to, no pressure.**

Even if you sponsor me, I probably won't treat you differently. I don't want to say "sponsor X amount and get Y". I don't want sponsorship to lose its original meaning.

Keep it pure!
