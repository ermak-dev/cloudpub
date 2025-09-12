document.addEventListener('DOMContentLoaded', function() {
  const fifoContainer = document.getElementById('fifo-animation');
  const fairContainer = document.getElementById('fair-animation');

  if (!fifoContainer || !fairContainer) return;

  // Configuration
  const config = {
    queueLength: 10,
    packetTypes: 3,
    animationSpeed: 1000,
    packetSize: 30,
    gap: 5
  };

  // Initialize queues
  initQueue(fifoContainer, 'FIFO');
  initQueue(fairContainer, 'Fair');

  // Start animations
  setTimeout(() => {
    animateFifo(fifoContainer, config);
    animateFair(fairContainer, config);
  }, 500);

  function initQueue(container, type) {
    // Create queue lanes
    const queueDiv = document.createElement('div');
    queueDiv.className = 'queue-container';

    // Create input area (left side)
    const inputArea = document.createElement('div');
    inputArea.className = 'input-area';
    for (let i = 0; i < config.packetTypes; i++) {
      const lane = document.createElement('div');
      lane.className = `input-lane packet-type-${i+1}`;
      lane.innerHTML = `<div class="lane-label">Поток ${i+1}</div>`;
      inputArea.appendChild(lane);
    }
    queueDiv.appendChild(inputArea);

    // Create queue area (middle)
    const queueArea = document.createElement('div');
    queueArea.className = 'queue-area';
    if (type === 'FIFO') {
      // FIFO has one lane
      const lane = document.createElement('div');
      lane.className = 'queue-lane fifo-lane';
      lane.innerHTML = '<div class="lane-label">Общая очередь</div>';
      queueArea.appendChild(lane);
    } else {
      // Fair has multiple lanes
      for (let i = 0; i < config.packetTypes; i++) {
        const lane = document.createElement('div');
        lane.className = `queue-lane fair-lane packet-type-${i+1}`;
        lane.innerHTML = `<div class="lane-label">Очередь ${i+1}</div>`;
        queueArea.appendChild(lane);
      }
    }
    queueDiv.appendChild(queueArea);

    // Create output area (right side)
    const outputArea = document.createElement('div');
    outputArea.className = 'output-area';
    outputArea.innerHTML = '<div class="output-label">Канал</div>';
    queueDiv.appendChild(outputArea);

    // Add title
    const title = document.createElement('h4');
    title.textContent = type === 'FIFO' ? 'Обычная очередь (FIFO)' : 'Справедливая очередь (Fair Queuing)';

    // Add explanation
    const explanation = document.createElement('p');
    explanation.className = 'queue-explanation';
    if (type === 'FIFO') {
      explanation.textContent = 'В обычной очереди "жадный" поток может занять большую часть канала';
    } else {
      explanation.textContent = 'В справедливой очереди каждый поток получает равную долю пропускной способности';
    }

    // Add everything to container
    container.appendChild(title);
    container.appendChild(queueDiv);
    container.appendChild(explanation);
  }

  function animateFifo(container, config) {
    const queueLane = container.querySelector('.fifo-lane');
    const outputArea = container.querySelector('.output-area');
    let packetCount = 0;

    // Generate packets at different rates
    // Type 1 (aggressive) generates 3x more packets
    setInterval(() => {
      if (Math.random() < 0.8) createPacket(1, queueLane, outputArea);
      packetCount++;
    }, config.animationSpeed / 3);

    // Type 2 (normal)
    setInterval(() => {
      if (Math.random() < 0.4) createPacket(2, queueLane, outputArea);
      packetCount++;
    }, config.animationSpeed);

    // Type 3 (normal)
    setInterval(() => {
      if (Math.random() < 0.4) createPacket(3, queueLane, outputArea);
      packetCount++;
    }, config.animationSpeed);

    // Process queue
    setInterval(() => {
      processQueue(queueLane, outputArea);
    }, config.animationSpeed / 2);

    function createPacket(type, queueLane, outputArea) {
      const packet = document.createElement('div');
      packet.className = `packet packet-type-${type}`;
      packet.dataset.type = type;
      queueLane.appendChild(packet);

      // Remove packets if too many
      const packets = queueLane.querySelectorAll('.packet');
      if (packets.length > config.queueLength) {
        packets[0].remove();
      }
    }

    function processQueue(queueLane, outputArea) {
      const packet = queueLane.querySelector('.packet');
      if (packet) {
        const type = packet.dataset.type;
        const outputPacket = packet.cloneNode(true);
        packet.remove();

        outputArea.appendChild(outputPacket);
        setTimeout(() => {
          outputPacket.style.opacity = 0;
          setTimeout(() => outputPacket.remove(), 500);
        }, 500);
      }
    }
  }

  function animateFair(container, config) {
    const queueLanes = container.querySelectorAll('.fair-lane');
    const outputArea = container.querySelector('.output-area');
    let currentLane = 0;

    // Generate packets at different rates
    // Type 1 (aggressive) generates 3x more packets
    setInterval(() => {
      if (Math.random() < 0.8) createPacket(1, queueLanes[0]);
    }, config.animationSpeed / 3);

    // Type 2 (normal)
    setInterval(() => {
      if (Math.random() < 0.4) createPacket(2, queueLanes[1]);
    }, config.animationSpeed);

    // Type 3 (normal)
    setInterval(() => {
      if (Math.random() < 0.4) createPacket(3, queueLanes[2]);
    }, config.animationSpeed);

    // Process queue in round-robin fashion
    setInterval(() => {
      processQueue(queueLanes[currentLane], outputArea);
      currentLane = (currentLane + 1) % queueLanes.length;
    }, config.animationSpeed / 2);

    function createPacket(type, queueLane) {
      const packet = document.createElement('div');
      packet.className = `packet packet-type-${type}`;
      packet.dataset.type = type;
      queueLane.appendChild(packet);

      // Remove packets if too many
      const packets = queueLane.querySelectorAll('.packet');
      if (packets.length > config.queueLength) {
        packets[0].remove();
      }
    }

    function processQueue(queueLane, outputArea) {
      const packet = queueLane.querySelector('.packet');
      if (packet) {
        const type = packet.dataset.type;
        const outputPacket = packet.cloneNode(true);
        packet.remove();

        outputArea.appendChild(outputPacket);
        setTimeout(() => {
          outputPacket.style.opacity = 0;
          setTimeout(() => outputPacket.remove(), 500);
        }, 500);
      }
    }
  }
});
