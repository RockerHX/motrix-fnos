<template>
  <section class="extensions-page">
    <div class="extensions-card">
      <span class="extensions-icon" aria-hidden="true">♧</span>
      <p class="eyebrow">Extensions</p>
      <h1>扩展功能暂未开放</h1>
      <p>
        当前 FPK Web 版尚未提供插件运行时。后续如果接入扩展能力，会优先通过受控的后端接口和明确的安全边界开放。
      </p>
      <ul>
        <li>不会在本阶段加载第三方脚本。</li>
        <li>不会联网拉取插件或执行外部代码。</li>
        <li>这里会作为后续扩展能力的入口。</li>
      </ul>
    </div>
  </section>
</template>

<style scoped>
.extensions-page {
  height: 100%;
  display: grid;
  place-items: center;
  padding: 24px;
}

.extensions-card {
  width: min(560px, 100%);
  border: 1px solid #344238;
  border-radius: 16px;
  padding: 34px;
  color: #d7dfd8;
  background: linear-gradient(145deg, rgba(27, 35, 29, 0.96), rgba(18, 20, 18, 0.96));
  box-shadow: 0 18px 50px rgba(0, 0, 0, 0.3);
}

.extensions-icon {
  width: 54px;
  height: 54px;
  display: grid;
  place-items: center;
  border-radius: 14px;
  color: #101710;
  background: #68ae5a;
  font-size: 30px;
  font-weight: 800;
}

.eyebrow {
  margin: 22px 0 8px;
  color: #8ef08a;
  font-size: 13px;
  font-weight: 700;
  letter-spacing: 0.14em;
  text-transform: uppercase;
}

h1 {
  margin: 0 0 14px;
  color: #f1f2ed;
  font-size: 26px;
  font-weight: 500;
}

p {
  margin: 0;
  color: #b7bfb4;
  line-height: 1.7;
}

ul {
  margin: 22px 0 0;
  padding-left: 20px;
  color: #c9d3c6;
  line-height: 1.8;
}
</style>
