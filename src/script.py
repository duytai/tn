import sys
import yaml
import copy
import os
from types import ModuleType
from importlib import import_module
from typing import Dict, Any, List
from pathlib import Path
from rich.console import Console
from rich import print_json

console = Console()

def component_from_module_path(path: str):
    if path == '':
        raise ValueError('Empty path')
    parts = [part for part in path.split('.')]
    for part in parts:
        # If a relative path is passed in, the first part will be empty
        if not len(part):
            raise ValueError(
                f"Error loading '{path}': invalid dotstring."
                + "\nRelative imports are not supported."
            )
    # First module requires trying to import to validate
    part0 = parts[0]
    try:
        obj = import_module(part0)
    except ImportError as exc_import:
        raise ValueError(
            f"Error loading '{path}':\n{repr(exc_import)}"
            + f"\nAre you sure that module '{part0}' is installed?"
        ) from exc_import
    # Subsequent components can be checked via getattr() on first module
    # It can either be an attribute that we can return or a submodule that we
    # can import and continue searching
    for m in range(1, len(parts)):
        part = parts[m]
        try:
            obj = getattr(obj, part)
        # If getattr fails, check to see if it's a module we can import and
        # continue down the path
        except AttributeError as exc_attr:
            parent_dotpath = '.'.join(parts[:m])
            if isinstance(obj, ModuleType):
                mod = '.'.join(parts[: m + 1])
                try:
                    obj = import_module(mod)
                    continue
                except ModuleNotFoundError as exc_import:
                    raise ValueError(
                        f"Error loading '{path}':\n{repr(exc_import)}"
                        + f"\nAre you sure that '{part}' is importable from module '{parent_dotpath}'?"
                    ) from exc_import
                # Any other error trying to import module can be raised as
                # InstantiationError
                except Exception as exc_import:
                    raise ValueError(
                        f"Error loading '{path}':\n{repr(exc_import)}"
                    ) from exc_import
            # If the component is not an attribute nor a module, it doesn't exist
            raise ValueError(
                f"Error loading '{path}':\n{repr(exc_attr)}"
                + f"\nAre you sure that '{part}' is an attribute of '{parent_dotpath}'?"
            ) from exc_attr
    return obj

def merge_dot_path(base_dict: Dict, dot_path: str, value: Any) -> None:
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

        # resolve relative module paths
        sys.path.append(str((Path.cwd() / yaml_file).parent))

        if '_sweep_' not in config:
            conf_str = yaml.safe_dump(config, default_flow_style=False)
            return [conf_str]

        output = visit(config['_sweep_'])()
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
    except Exception as _e:
        console.print_exception(show_locals=True)
    return []

def visit_yaml(f: str) -> Any:
    return visit(read_yaml(f))

def read_yaml(f: str) -> Dict:
    return yaml.safe_load(read_text(f))

def read_text(f: str) -> str:
    return Path(f).read_text()

def visit(el: Any) -> Any:
    if isinstance(el, dict):
        el = dict([(k, visit(v)) for k, v in el.items()])
        if '_component_' in el:
            module_path = el['_component_'].strip()
            if module_path in globals():
                component = globals().get(module_path)
            else:
                component = component_from_module_path(module_path)
            args = el.get('_args_', [])
            kwargs = dict([(k, v) for k, v in el.items() if not k.startswith('_') and not k.endswith('_')])
            return component(*args, **kwargs)
        return el
    elif isinstance(el, list):
        return [visit(x) for x in el]
    elif isinstance(el, (str, int, float, bool)) or el is None:
        return el
    else:
        raise ValueError(f'unknown type: {el}')

def execute(text: str, sweep_only: bool) -> None:
    try:
        os.environ['CONFIG'] = text
        config = yaml.safe_load(text)
        print_json(data=config)
        if '_component_' not in config or sweep_only:
            return
        visit(config)()
    except Exception as _e:
        console.print_exception(show_locals=True)