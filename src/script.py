import sys
import yaml
import importlib
import copy
import json
import traceback
from rich import print
from typing import Dict, Any, List
from pathlib import Path

def merge_dot_path(
    base_dict: Dict,
    dot_path: str,
    value: Any
) -> None:
    keys = dot_path.split('.')
    current = base_dict

    for key in keys[:-1]:
        if key.endswith(']'):
            # Extract array index
            array_key, index = key[:-1].split('[')
            index = int(index)
            if array_key not in current:
                current[array_key] = []
            while len(current[array_key]) <= index:
                current[array_key].append({})
            current = current[array_key][index]
        else:
            if key not in current:
                current[key] = {}
            current = current[key]

    last_key = keys[-1]
    if last_key.endswith(']'):
        array_key, index = last_key[:-1].split('[')
        index = int(index)
        if array_key not in current:
            current[array_key] = []
        while len(current[array_key]) <= index:
            current[array_key].append(None)
        current[array_key][index] = value
    else:
        current[last_key] = value

def sweep(yaml_file: str, project_dir: str) -> List[str]:
    try:
        if project_dir not in sys.path:
            sys.path.append(project_dir)
        config = yaml.safe_load(Path(yaml_file).read_text())
        if '_sweep_' not in config:
            conf_str = yaml.safe_dump(config, default_flow_style=False)
            return [conf_str]

        module_path = config['_sweep_'].strip()
        if '.' not in module_path or module_path.startswith('.') or module_path.endswith('.'):
            raise ValueError(f'invalid module path: {module_path}')
        pos = module_path.rfind('.')
        module = importlib.import_module(module_path[:pos])
        component = getattr(module, module_path[pos + 1:])
        output = component()()
        if not isinstance(output, list):
            output = []

        result = []
        for el in output:
            tmp = copy.deepcopy(config)
            for dotpath, value in el.items():
                merge_dot_path(tmp, dotpath, value)
            conf_str = yaml.safe_dump(tmp, default_flow_style=False)
            result.append(conf_str)
        return result
    except Exception as e:
        traceback.print_exc()
    return []

def visit(el: Any) -> Any:
    if isinstance(el, dict):
        el = dict([(k, visit(v)) for k, v in el.items()])
        if '_component_' in el:
            module_path = el['_component_'].strip()
            if '.' not in module_path or module_path.startswith('.') or module_path.endswith('.'):
                raise ValueError(f'invalid module path: {module_path}')
            pos = module_path.rfind('.')
            module = importlib.import_module(module_path[:pos])
            component = getattr(module, module_path[pos + 1:])
            kwargs = dict([(k, v) for k, v in el.items() if not k.startswith('_') and not k.endswith('_')])
            return component(**kwargs)()
        return el
    elif isinstance(el, list):
        return [visit(x) for x in el]
    elif isinstance(el, (str, int, float, bool)) or el is None:
        return el
    else:
        raise ValueError(f'unknown type: {el}')

def execute(text: str, project_dir: str) -> None:
    try:
        if project_dir not in sys.path:
            sys.path.append(project_dir)
        config = yaml.safe_load(text)
        print(json.dumps(config, indent=2))
        if '_component_' not in config:
            return
        visit(config)
    except Exception as e:
        traceback.print_exc()