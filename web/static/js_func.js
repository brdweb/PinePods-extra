// // Truncate or expand based on the element's max-height
// function toggleDescription(guid, expanded) {
//     const content = document.querySelector(`.desc-${guid}`);
//     if (expanded) {
//         content.style.maxHeight = "none";  // Expand fully
//     } else {
//         content.style.maxHeight = "100px"; // Truncate (set to desired truncation height)
//     }
// }

document.addEventListener("DOMContentLoaded", () => {
  const allDescriptions = document.querySelectorAll('[id^="desc-"]');
  allDescriptions.forEach((desc) => {
    const id = desc.id.replace("desc-", "");
    toggleDescription(id, false); // Start all as collapsed
  });
});

window.toggleDescription = function (guid, shouldExpand) {
  const descContainer = document.querySelector(`.desc-${guid}`);
  if (descContainer) {
    if (shouldExpand) {
      descContainer.classList.add("expanded");
      descContainer.classList.remove("collapsed");
    } else {
      descContainer.classList.remove("expanded");
      descContainer.classList.add("collapsed");
    }
  } else {
    console.error("Description container not found for GUID: " + guid);
  }
};

window.addEventListener("load", function () {
  const descriptions = document.querySelectorAll(
    ".episode-description-container",
  );
  descriptions.forEach((desc) => {
    const btn = desc.nextElementSibling; // Assuming button is the next sibling
    if (desc.scrollHeight > desc.clientHeight) {
      if (btn) {
        btn.classList.remove("hidden"); // Show button if content is clipped
      }
    } else {
      if (btn) {
        btn.classList.add("hidden"); // Hide button if content is not clipped
      }
    }
  });
});

function toggle_description(guid) {
  const selector = `#${guid.replace(/[^a-zA-Z0-9-]/g, "")}`; // Use ID selector
  console.log("Trying to select:", selector);
  const descContainer = document.querySelector(selector);

  if (!descContainer) {
    console.error(
      "Description container not found for GUID: " +
        guid +
        " with selector: " +
        selector,
    );
    return;
  }

  const button = descContainer.querySelector(".toggle-desc-btn");
  if (!button) {
    console.error(
      "Button not found in description container for GUID: " + guid,
    );
    return;
  }

  if (descContainer.classList.contains("desc-collapsed")) {
    descContainer.classList.remove("desc-collapsed");
    descContainer.classList.add("desc-expanded");
    button.textContent = "";
  } else {
    descContainer.classList.add("desc-collapsed");
    descContainer.classList.remove("desc-expanded");
    button.textContent = "";
  }
}

// Attach a progressive file or an HLS playlist. hls.js is vendored so a homelab
// does not need a CDN. Safari plays HLS natively. A fatal hls.js error becomes a
// media error so the player can try the next source.
window.pinepodsAttachMedia = function (media, url, hls) {
  if (media._pinepodsHls) {
    try {
      media._pinepodsHls.destroy();
    } catch (e) {}
    media._pinepodsHls = null;
  }
  if (!url) {
    media.removeAttribute("src");
    try {
      media.load();
    } catch (e) {}
    return false;
  }
  if (!hls) {
    media.src = url;
    return true;
  }
  const native = media.canPlayType("application/vnd.apple.mpegurl");
  if (native === "probably" || native === "maybe") {
    media.src = url;
    return true;
  }
  if (window.Hls && window.Hls.isSupported()) {
    const player = new window.Hls({ enableWorker: false });
    player.loadSource(url);
    player.attachMedia(media);
    player.on(window.Hls.Events.ERROR, function (_event, data) {
      if (data && data.fatal) {
        media.dispatchEvent(new Event("error"));
      }
    });
    media._pinepodsHls = player;
    return true;
  }
  media.src = url;
  return false;
};
