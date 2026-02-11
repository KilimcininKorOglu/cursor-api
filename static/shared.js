// Token management functionality
/**
 * Save authentication token to local storage
 * @param {string} token - The authentication token to save
 * @returns {void}
 */
function saveAuthToken(token) {
  const expiryTime = new Date().getTime() + 24 * 60 * 60 * 1000; // Expires after 24 hours
  localStorage.setItem("authToken", token);
  localStorage.setItem("authTokenExpiry", expiryTime);
}

/**
 * Get stored authentication token
 * @returns {string|null} Returns token if valid, otherwise returns null
 */
function getAuthToken() {
  const token = localStorage.getItem("authToken");
  const expiry = localStorage.getItem("authTokenExpiry");

  if (!token || !expiry) {
    return null;
  }

  if (new Date().getTime() > parseInt(expiry)) {
    localStorage.removeItem("authToken");
    localStorage.removeItem("authTokenExpiry");
    return null;
  }

  return token;
}

// Message display functionality
/**
 * Display message in specified element
 * @param {string} elementId - Target element ID
 * @param {string} text - Message text to display
 * @param {boolean} [isError=false] - Whether it's an error message
 * @returns {void}
 */
function showMessage(elementId, text, isError = false) {
  let msg = document.getElementById(elementId);

  // If message element doesn't exist, create a new one
  if (!msg) {
    msg = document.createElement("div");
    msg.id = elementId;
    document.body.appendChild(msg);
  }

  msg.className = `floating-message ${isError ? "error" : "success"}`;
  msg.innerHTML = text.replace(/\n/g, "<br>");
}

// Ensure message container exists
/**
 * Ensure message container exists in DOM
 * @returns {HTMLElement} Message container element
 */
function ensureMessageContainer() {
  let container = document.querySelector(".message-container");
  if (!container) {
    container = document.createElement("div");
    container.className = "message-container";
    document.body.appendChild(container);
  }
  return container;
}

/**
 * Display global message notification
 * @param {string} text - Message text to display
 * @param {boolean} [isError=false] - Whether it's an error message
 * @param {number} [timeout=3000] - Message display duration (milliseconds)
 * @returns {void}
 */
function showGlobalMessage(text, isError = false, timeout = 3000) {
  const container = ensureMessageContainer();

  const msgElement = document.createElement("div");
  msgElement.className = `message ${isError ? "error" : "success"}`;
  msgElement.textContent = text;

  container.appendChild(msgElement);

  // Set fade-out animation and removal
  setTimeout(() => {
    msgElement.style.animation = "messageOut 0.3s ease-in-out";
    setTimeout(() => {
      msgElement.remove();
      // If container is empty, also remove container
      if (container.children.length === 0) {
        container.remove();
      }
    }, 300);
  }, timeout);
}

// Token input auto-fill and event binding
function initializeTokenHandling(inputId) {
  // Try to fill directly, if DOM not ready will try again in event
  const tryFillToken = () => {
    const tokenInput = document.getElementById(inputId);
    if (tokenInput) {
      const authToken = getAuthToken();
      if (authToken) {
        tokenInput.value = authToken;
      }

      // Bind change event
      tokenInput.addEventListener("change", (e) => {
        if (e.target.value) {
          saveAuthToken(e.target.value);
        } else {
          localStorage.removeItem("authToken");
          localStorage.removeItem("authTokenExpiry");
        }
      });

      return true;
    }
    return false;
  };

  // Try to execute immediately
  if (!tryFillToken()) {
    // If element doesn't exist yet, wait for DOM to load
    if (document.readyState === 'loading') {
      document.addEventListener("DOMContentLoaded", tryFillToken);
    } else {
      // DOM loaded but element doesn't exist, may need to wait a bit
      setTimeout(tryFillToken, 0);
    }
  }
}

// API request common handling
async function makeAuthenticatedRequest(url, options = {}) {
  const tokenId = options.tokenId || "authToken";
  const token = document.getElementById(tokenId).value;

  if (!token) {
    showGlobalMessage("Please enter AUTH_TOKEN", true);
    return null;
  }

  if (!/^[A-Za-z0-9\-._~+/]+=*$/.test(token)) {
    showGlobalMessage("Invalid TOKEN format, please check for special characters", true);
    return null;
  }

  const defaultOptions = {
    method: "POST",
    headers: {
      Authorization: `Bearer ${token}`,
      "Content-Type": "application/json",
    },
  };

  try {
    const response = await fetch(url, { ...defaultOptions, ...options });

    if (!response.ok) {
      throw new Error(`HTTP error! status: ${response.status}`);
    }

    return await response.json();
  } catch (error) {
    showGlobalMessage(`Request failed: ${error.message}`, true);
    return null;
  }
}

/**
 * Parse boolean value from string
 * @param {string} str - String to parse
 * @param {boolean|null} defaultValue - Default value when parsing fails
 * @returns {boolean|null} Parse result, returns default value if unable to parse
 */
function parseBooleanFromString(str, defaultValue = null) {
  if (typeof str !== "string") {
    return defaultValue;
  }

  const lowercaseStr = str.toLowerCase().trim();

  if (lowercaseStr === "true" || lowercaseStr === "1") {
    return true;
  } else if (lowercaseStr === "false" || lowercaseStr === "0") {
    return false;
  } else {
    return defaultValue;
  }
}

/**
 * Convert boolean value to string
 * @param {boolean|undefined|null} value - Boolean value to convert
 * @param {string} defaultValue - Default value when conversion fails
 * @returns {string} Conversion result, returns default value if input is invalid
 */
function parseStringFromBoolean(value, defaultValue = null) {
  if (typeof value !== "boolean") {
    return defaultValue;
  }

  return value ? "true" : "false";
}

/**
 * Convert membership type code to display name
 * @param {string|null} type - Membership type code, e.g. 'free_trial', 'pro', 'free', 'enterprise'
 * @returns {string} Formatted membership type display name
 * @example
 * formatMembershipType('free_trial') // Returns 'Pro Trial'
 * formatMembershipType('pro') // Returns 'Pro'
 * formatMembershipType(null) // Returns '-'
 * formatMembershipType('custom_type') // Returns 'Custom Type'
 */
function formatMembershipType(type) {
  if (!type) return "-";
  switch (type) {
    case "free_trial":
      return "Pro Trial";
    case "pro":
      return "Pro";
    case "free":
      return "Free";
    case "enterprise":
      return "Business";
    default:
      return type
        .split("_")
        .map((word) => word.charAt(0).toUpperCase() + word.slice(1))
        .join(" ");
  }
}

// Copy text functionality
/**
 * Copy text to clipboard
 * @param {string} text - Text to copy
 * @param {Object} [options={}] - Configuration options
 * @param {boolean} [options.showMessage=true] - Whether to show copy result message
 * @param {string} [options.successMessage='Copied to clipboard'] - Success message
 * @param {string} [options.errorMessage='Copy failed, please copy manually'] - Error message
 * @param {Function} [options.onSuccess] - Callback on successful copy
 * @param {Function} [options.onError] - Callback on failed copy
 * @param {HTMLElement} [options.sourceElement] - Source element that triggered copy (for temporary state display)
 * @returns {Promise<boolean>} Returns whether copy was successful
 * @example
 * // Basic usage
 * copyToClipboard('Hello World');
 *
 * // Custom messages
 * copyToClipboard('proxy address', {
 *   successMessage: 'Proxy address copied',
 *   errorMessage: 'Unable to copy proxy address'
 * });
 *
 * // With callback functions
 * copyToClipboard('sensitive info', {
 *   showMessage: false,
 *   onSuccess: () => console.log('Copy successful'),
 *   onError: (err) => console.error('Copy failed:', err)
 * });
 *
 * // Use with button
 * const button = document.getElementById('copyBtn');
 * copyToClipboard('text content', { sourceElement: button });
 */
async function copyToClipboard(text, options = {}) {
  const {
    showMessage = true,
    successMessage = "Copied to clipboard",
    errorMessage = "Copy failed, please copy manually",
    onSuccess,
    onError,
    sourceElement,
  } = options;

  // Validate input
  if (typeof text !== "string") {
    console.error("copyToClipboard: Text must be string type");
    if (showMessage) {
      showGlobalMessage("Invalid copy content", true);
    }
    if (onError) {
      onError(new Error("Invalid text type"));
    }
    return false;
  }

  // If text is empty, give warning
  if (!text.trim()) {
    console.warn("copyToClipboard: Attempting to copy empty text");
    if (showMessage) {
      showGlobalMessage("No content to copy", true);
    }
    if (onError) {
      onError(new Error("Empty text"));
    }
    return false;
  }

  try {
    // Prefer modern Clipboard API
    if (navigator.clipboard && window.isSecureContext) {
      await navigator.clipboard.writeText(text);
      handleCopySuccess();
      return true;
    } else {
      // Fallback to traditional method
      const success = fallbackCopyToClipboard(text);
      if (success) {
        handleCopySuccess();
        return true;
      } else {
        throw new Error("Fallback copy failed");
      }
    }
  } catch (error) {
    console.error("Copy to clipboard failed:", error);

    if (showMessage) {
      showGlobalMessage(errorMessage, true);
    }

    if (onError) {
      onError(error);
    }

    return false;
  }

  // Handle copy success
  function handleCopySuccess() {
    if (showMessage) {
      showGlobalMessage(successMessage);
    }

    if (onSuccess) {
      onSuccess();
    }

    // If source element provided, can add temporary visual feedback
    if (sourceElement) {
      addTemporaryClass(sourceElement, "copied", 2000);
    }
  }
}

/**
 * Traditional copy method (for browsers that don't support Clipboard API)
 * @private
 * @param {string} text - Text to copy
 * @returns {boolean} Whether copy was successful
 */
function fallbackCopyToClipboard(text) {
  // Create temporary text area
  const textArea = document.createElement("textarea");

  // Set styles to make it invisible but copyable
  textArea.value = text;
  textArea.style.position = "fixed";
  textArea.style.top = "0";
  textArea.style.left = "0";
  textArea.style.width = "2em";
  textArea.style.height = "2em";
  textArea.style.padding = "0";
  textArea.style.border = "none";
  textArea.style.outline = "none";
  textArea.style.boxShadow = "none";
  textArea.style.background = "transparent";
  textArea.style.opacity = "0";
  textArea.style.pointerEvents = "none";

  // Prevent zoom on mobile devices
  textArea.style.fontSize = "12pt";

  document.body.appendChild(textArea);

  try {
    // Select text
    textArea.select();
    textArea.setSelectionRange(0, text.length);

    // Execute copy
    const successful = document.execCommand("copy");

    // Cleanup
    document.body.removeChild(textArea);

    return successful;
  } catch (error) {
    console.error("Traditional copy method failed:", error);
    // Ensure cleanup
    if (document.body.contains(textArea)) {
      document.body.removeChild(textArea);
    }
    return false;
  }
}

/**
 * Temporarily add CSS class to element
 * @private
 * @param {HTMLElement} element - Target element
 * @param {string} className - Class name to add
 * @param {number} duration - Duration (milliseconds)
 */
function addTemporaryClass(element, className, duration) {
  if (!element || !className) return;

  element.classList.add(className);
  setTimeout(() => {
    element.classList.remove(className);
  }, duration);
}

/**
 * Copy table cell content
 * @param {HTMLElement} cell - Table cell element
 * @param {Object} [options={}] - Copy options (same as copyToClipboard)
 * @returns {Promise<boolean>} Whether copy was successful
 * @example
 * // Use in table cell click event
 * td.onclick = () => copyTableCellContent(td);
 */
async function copyTableCellContent(cell, options = {}) {
  if (!cell) {
    console.error("copyTableCellContent: No valid cell element provided");
    return false;
  }

  // Get plain text content (remove HTML tags)
  const text = cell.textContent || cell.innerText || "";

  return copyToClipboard(text.trim(), {
    ...options,
    sourceElement: cell,
  });
}

/**
 * Create button with copy functionality
 * @param {string} text - Text to copy
 * @param {Object} [options={}] - Button configuration options
 * @param {string} [options.buttonText='Copy'] - Button text
 * @param {string} [options.buttonClass='copy-button'] - Button CSS class
 * @param {string} [options.copiedText='Copied'] - Button text after successful copy
 * @param {number} [options.resetDelay=2000] - Button text reset delay (milliseconds)
 * @returns {HTMLButtonElement} Created button element
 * @example
 * // Create a copy button
 * const copyBtn = createCopyButton('text to copy', {
 *   buttonText: 'Copy Key',
 *   copiedText: '✓ Copied'
 * });
 * document.getElementById('container').appendChild(copyBtn);
 */
function createCopyButton(text, options = {}) {
  const {
    buttonText = "Copy",
    buttonClass = "copy-button",
    copiedText = "Copied",
    resetDelay = 2000,
  } = options;

  const button = document.createElement("button");
  button.textContent = buttonText;
  button.className = buttonClass;
  button.type = "button";

  button.addEventListener("click", async () => {
    const originalText = button.textContent;

    const success = await copyToClipboard(text, {
      sourceElement: button,
      showMessage: true,
    });

    if (success) {
      button.textContent = copiedText;
      button.disabled = true;

      setTimeout(() => {
        button.textContent = originalText;
        button.disabled = false;
      }, resetDelay);
    }
  });

  return button;
}

/**
 * Check if Clipboard API is available
 * @returns {boolean} Whether Clipboard API is supported
 * @example
 * if (isClipboardSupported()) {
 *   console.log('Browser supports modern Clipboard API');
 * }
 */
function isClipboardSupported() {
  return !!(navigator.clipboard && window.isSecureContext);
}

/**
 * Read text from clipboard (requires user permission)
 * @param {Object} [options={}] - Configuration options
 * @param {boolean} [options.showMessage=true] - Whether to show result message
 * @param {Function} [options.onSuccess] - Callback on successful read
 * @param {Function} [options.onError] - Callback on failed read
 * @returns {Promise<string|null>} Text from clipboard, returns null on failure
 * @example
 * const text = await readFromClipboard();
 * if (text) {
 *   console.log('Clipboard content:', text);
 * }
 */
async function readFromClipboard(options = {}) {
  const { showMessage = true, onSuccess, onError } = options;

  if (!isClipboardSupported()) {
    const error = new Error("Browser does not support Clipboard API");
    if (showMessage) {
      showGlobalMessage("Browser does not support reading clipboard", true);
    }
    if (onError) {
      onError(error);
    }
    return null;
  }

  try {
    const text = await navigator.clipboard.readText();

    if (onSuccess) {
      onSuccess(text);
    }

    return text;
  } catch (error) {
    console.error("Read clipboard failed:", error);

    if (showMessage) {
      if (error.name === "NotAllowedError") {
        showGlobalMessage("Permission required to read clipboard", true);
      } else {
        showGlobalMessage("Unable to read clipboard content", true);
      }
    }

    if (onError) {
      onError(error);
    }

    return null;
  }
}
