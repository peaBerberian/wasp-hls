export function createBreakElement() {
  return createElement("br", {});
}

export function createContainerElt(elementName, props, innerElements) {
  const elt = document.createElement(elementName);
  for (let key in props) {
    elt[key] = props[key];
  }
  addInnerElements(elt, innerElements);
  return elt;
}

export function createElement(elementName, props) {
  const elt = document.createElement(elementName);
  for (let key in props) {
    elt[key] = props[key];
  }
  return elt;
}

export function addInnerElements(parentElement, innerElements) {
  for (let innerElt of innerElements) {
    parentElement.appendChild(innerElt);
  }
}
